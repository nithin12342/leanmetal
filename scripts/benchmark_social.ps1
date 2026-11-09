# scripts/benchmark_social.ps1 — Arjay McCandless $12 Server Benchmark Load Runner
# Runs the benchmark user journey against the isolated container (1 vCPU, 2048 MB RAM)
# 40% Profile Reads (GET /users/1)
# 40% Timeline Reads (GET /posts?limit=20)
# 20% Mutations (POST /posts)

param(
  [string]$Base = "http://127.0.0.1:8081",
  [int]$Requests = 2500,
  [int]$Concurrency = 50
)

$code = @"
using System;
using System.Diagnostics;
using System.Net.Http;
using System.Text;
using System.Threading;
using System.Threading.Tasks;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Linq;

public class SocialBenchmark {
    public static void Execute(string baseUrl, int totalRequests, int maxConcurrency) {
        Console.WriteLine("============================================================");
        Console.WriteLine("Starting Benchmark Run against Isolated Container ($12 VPS)");
        Console.WriteLine(string.Format("Target Endpoint: {0} (1 vCPU, 2048 MB RAM cgroup limit)", baseUrl));
        Console.WriteLine("Workload: 40% Profile Reads, 40% Timeline Feeds, 20% Mutations");
        Console.WriteLine(string.Format("Total Operations: {0} | Concurrency: {1}", totalRequests, maxConcurrency));
        Console.WriteLine("============================================================");

        var handler = new HttpClientHandler {
            MaxConnectionsPerServer = 2000
        };
        var client = new HttpClient(handler) {
            Timeout = TimeSpan.FromSeconds(10)
        };

        var throttler = new SemaphoreSlim(maxConcurrency);
        var latencies = new ConcurrentBag<double>();
        int ok = 0;
        int fail = 0;

        var swTotal = Stopwatch.StartNew();
        var tasks = new List<Task>();

        for (int i = 0; i < totalRequests; i++) {
            throttler.Wait();
            int reqId = i;

            tasks.Add(Task.Run(async () => {
                var sw = Stopwatch.StartNew();
                try {
                    int roll = reqId % 10;
                    HttpResponseMessage resp;

                    if (roll < 4) {
                        // 1. Profile Read (40%)
                        resp = await client.GetAsync(baseUrl + "/users/1");
                    } else if (roll < 8) {
                        // 2. Timeline Feed Read (40%)
                        resp = await client.GetAsync(baseUrl + "/posts?limit=20&offset=0");
                    } else {
                        // 3. Post Creation / Mutation (20%)
                        string json = "{\"author_id\":1,\"content\":\"Load test post " + reqId + "\"}";
                        var content = new StringContent(json, Encoding.UTF8, "application/json");
                        resp = await client.PostAsync(baseUrl + "/posts", content);
                    }

                    sw.Stop();
                    latencies.Add(sw.Elapsed.TotalMilliseconds);

                    if (resp.IsSuccessStatusCode) {
                        Interlocked.Increment(ref ok);
                    } else {
                        Interlocked.Increment(ref fail);
                    }
                } catch {
                    Interlocked.Increment(ref fail);
                } finally {
                    throttler.Release();
                }
            }));
        }

        Task.WaitAll(tasks.ToArray());
        swTotal.Stop();

        double elapsedSec = swTotal.Elapsed.TotalSeconds;
        double rps = ok / elapsedSec;
        double errRate = ((double)fail / totalRequests) * 100.0;

        var sortedLat = latencies.OrderBy(x => x).ToList();
        double p50 = sortedLat.Count > 0 ? sortedLat[(int)(0.50 * (sortedLat.Count - 1))] : 0;
        double p90 = sortedLat.Count > 0 ? sortedLat[(int)(0.90 * (sortedLat.Count - 1))] : 0;
        double p95 = sortedLat.Count > 0 ? sortedLat[(int)(0.95 * (sortedLat.Count - 1))] : 0;
        double p99 = sortedLat.Count > 0 ? sortedLat[(int)(0.99 * (sortedLat.Count - 1))] : 0;
        double avg = sortedLat.Count > 0 ? sortedLat.Average() : 0;

        Console.WriteLine("\n============================================================");
        Console.WriteLine("BENCHMARK RESULTS ($12 Virtual Machine Hardware Envelope):");
        Console.WriteLine(string.Format("  Total Operations:   {0}", totalRequests));
        Console.WriteLine(string.Format("  Successful (2xx):   {0}", ok));
        Console.WriteLine(string.Format("  Failed:             {0}", fail));
        Console.WriteLine(string.Format("  Elapsed Time:       {0:N2} seconds", elapsedSec));
        Console.WriteLine(string.Format("  Throughput:         {0:N2} requests/sec", rps));
        Console.WriteLine(string.Format("  Error Rate:         {0:N2}% (SLA Budget: < 1.00%)", errRate));
        Console.WriteLine(string.Format("  Average Latency:    {0:N2} ms", avg));
        Console.WriteLine(string.Format("  P50 Latency:        {0:N2} ms", p50));
        Console.WriteLine(string.Format("  P90 Latency:        {0:N2} ms", p90));
        Console.WriteLine(string.Format("  P95 Latency:        {0:N2} ms (SLA Budget: < 1,000 ms)", p95));
        Console.WriteLine(string.Format("  P99 Latency:        {0:N2} ms", p99));
        Console.WriteLine("============================================================");

        if (p95 < 1000.0 && errRate < 1.0) {
            Console.WriteLine(">>> SLA VERDICT: PASSED (Hardware Budget Satisfied) <<<");
        } else {
            Console.WriteLine(">>> SLA VERDICT: FAILED (Exceeded Latency or Error Budget) <<<");
        }
    }
}
"@

Add-Type -ReferencedAssemblies "System.Net.Http" -TypeDefinition $code -Language CSharp
[SocialBenchmark]::Execute($Base, $Requests, $Concurrency)
