# scripts/smoke_traffic.ps1 — EdgeFlag traffic simulator (smoke + load).
# Seeds flags, fires mixed L1/dynamic evaluation traffic, prints latency
# percentiles and server-side /metrics counters.
param(
  [string]$Base = "http://127.0.0.1:8080",
  [string]$AdminToken = "edgeflag-admin-secret",
  [int]$Requests = 600
)

Add-Type -AssemblyName System.Net.Http
$http = [System.Net.Http.HttpClient]::new()

function Req($method, $path, $body, $auth = $false) {
  $msg = [System.Net.Http.HttpRequestMessage]::new($method, "$Base$path")
  if ($body) {
    $msg.Content = [System.Net.Http.StringContent]::new($body, [Text.Encoding]::UTF8, "application/json")
  }
  if ($auth) { $msg.Headers.Add("Authorization", "Bearer $AdminToken") }
  $resp = $http.SendAsync($msg).GetAwaiter().GetResult()
  $text = $resp.Content.ReadAsStringAsync().GetAwaiter().GetResult()
  return @{ Status = [int]$resp.StatusCode; Body = $text }
}

# 1. Seed 3 flags (PUT requires Bearer auth).
$seeds = @(
  @{ key = "checkout_flow"; enabled = $true;  rules = @(); default_variant = "v1" },
  @{ key = "promo_banner";  enabled = $true;  rules = @(); default_variant = "on" },
  @{ key = "kill_search";    enabled = $false; rules = @(); default_variant = "off" }
)
foreach ($f in $seeds) {
  $r = Req "Put" "/v1/flags/$($f.key)" ($f | ConvertTo-Json -Compress) $true
  Write-Output "seed $($f.key): $($r.Status)"
}

# 2. Traffic: alternate L1-eligible (empty attrs) and dynamic (with attrs) evals.
$lat = [System.Collections.Generic.List[double]]::new()
$ok = 0; $fail = 0; $l1hits = 0
$sw = [Diagnostics.Stopwatch]::new()
for ($i = 0; $i -lt $Requests; $i++) {
  $flag = $seeds[$i % 3].key
  if ($i % 2 -eq 0) {
    $payload = "{`"flag_key`":`"$flag`",`"context`":{`"user_id`":`"u$i`",`"attributes`":{}}}"
  } else {
    $payload = "{`"flag_key`":`"$flag`",`"context`":{`"user_id`":`"u$i`",`"attributes`":{`"tier`":`"pro`"}}}"
  }
  $sw.Restart()
  try {
    $r = Req "Post" "/v1/evaluate" $payload
    $sw.Stop()
    if ($r.Status -eq 200) {
      $ok++
      $lat.Add($sw.Elapsed.TotalMilliseconds)
      if ($r.Body -match '"served_from_l1":true') { $l1hits++ }
    } else { $fail++ }
  } catch { $fail++ }
}
$sorted = $lat | Sort-Object
$p50 = $sorted[[int](0.50 * ($sorted.Count - 1))]
$p99 = $sorted[[int](0.99 * ($sorted.Count - 1))]
$avg = ($lat | Measure-Object -Average).Average
Write-Output ("evals: ok={0} fail={1} l1_hits={2} avg={3:N2}ms p50={4:N2}ms p99={5:N2}ms" -f $ok, $fail, $l1hits, $avg, $p50, $p99)

# 3. Server-side counters.
$m = (Req "Get" "/metrics" $null).Body
foreach ($s in "edgeflag_evaluations_total", "edgeflag_l1_cache_hits_total",
  "edgeflag_l1_cache_misses_total", "edgeflag_wal_queue_depth",
  "edgeflag_valkey_connected", "edgeflag_xdp_rx_packets_total") {
  ($m -split "`n" | Where-Object { $_ -match "^$s " }) | Write-Output
}
