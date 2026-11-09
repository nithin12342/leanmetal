# scripts/find_upper_bound.ps1 — Automated Binary Search for Maximum Concurrency Upper Bound
# Finds the exact maximum concurrent users sustainable on the 1 vCPU / 2 GB RAM container
# SLA Rules: P95 < 1,000 ms, Error Rate < 1.00%

param(
  [string]$Base = "http://127.0.0.1:8081",
  [int]$SamplesPerStage = 1500
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

public class ConcurrencyFinder {
    public class StageResult {
        public int Concurrency;
        public int Total;
        public int Success;
        public int Failed;
        public double ElapsedSeconds;
        public double Throughput;
        public double ErrorRate;
        public double P50;
        public double P95;
        public double P99;
        public bool Passed;
    }

    public static StageResult TestConcurrency(string baseUrl, int concurrency, int sampleCount) {
        var handler = new HttpClientHandler {
            MaxConnectionsPerServer = 10000
        };
        var client = new HttpClient(handler) {
            Timeout = TimeSpan.FromSeconds(5)
        };

        var throttler = new SemaphoreSlim(concurrency);
        var latencies = new ConcurrentBag<double>();
        int ok = 0;
        int fail = 0;

        var swTotal = Stopwatch.StartNew();
        var tasks = new List<Task>();

        for (int i = 0; i < sampleCount; i++) {
            throttler.Wait();
            int reqId = i;

            tasks.Add(Task.Run(async () => {
                var sw = Stopwatch.StartNew();
                try {
                    int roll = reqId % 100;
                    HttpResponseMessage resp;

                    if (roll < 50) {
                        // 50% GET /feed
                        resp = await client.GetAsync(baseUrl + "/feed");
                    } else if (roll < 92) {
                        // 42% GET /posts/:id
                        int pid = (reqId % 200) + 1;
                        resp = await client.GetAsync(baseUrl + "/posts/" + pid);
                    } else if (roll < 98) {
                        // 6% POST /posts/:id/like
                        int pid = (reqId % 200) + 1;
                        var emptyContent = new StringContent("", Encoding.UTF8, "application/json");
                        resp = await client.PostAsync(baseUrl + "/posts/" + pid + "/like", emptyContent);
                    } else {
                        // 2% POST /posts
                        var json = "{\"author_id\":1,\"content\":\"K6 Post " + reqId + "\"}";
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

        double elapsed = swTotal.Elapsed.TotalSeconds;
        double errRate = ((double)fail / sampleCount) * 100.0;
        var sorted = latencies.OrderBy(x => x).ToList();

        double p50 = sorted.Count > 0 ? sorted[(int)(0.50 * (sorted.Count - 1))] : 0;
        double p95 = sorted.Count > 0 ? sorted[(int)(0.95 * (sorted.Count - 1))] : 0;
        double p99 = sorted.Count > 0 ? sorted[(int)(0.99 * (sorted.Count - 1))] : 0;

        bool passed = (p95 < 1000.0) && (errRate < 1.0);

        return new StageResult {
            Concurrency = concurrency,
            Total = sampleCount,
            Success = ok,
            Failed = fail,
            ElapsedSeconds = elapsed,
            Throughput = ok / elapsed,
            ErrorRate = errRate,
            P50 = p50,
            P95 = p95,
            P99 = p99,
            Passed = passed
        };
    }

    public static void FindCeiling(string baseUrl, int sampleSize) {
        Console.WriteLine("============================================================");
        Console.WriteLine("AUTOMATED BINARY SEARCH: DISCOVERING CONCURRENCY UPPER BOUND");
        Console.WriteLine("Target: " + baseUrl + " (1 vCPU, 2048 MB RAM cgroup limit)");
        Console.WriteLine("SLA Boundary: P95 < 1,000 ms AND Error Rate < 1.00%");
        Console.WriteLine("============================================================\n");

        int low = 50;
        int high = 50;
        StageResult lastGood = null;

        // Step 1: Exponential Ramp to find the failure bracket [Low, High]
        Console.WriteLine("--- STAGE 1: EXPONENTIAL BRACKETING ---");
        int current = 50;
        while (current <= 32000) {
            Console.Write(string.Format("Testing Concurrency = {0,5} ... ", current));
            var res = TestConcurrency(baseUrl, current, sampleSize);

            Console.WriteLine(string.Format("Throughput: {0,7:N0} RPS | P95: {1,6:N1} ms | Err: {2,4:N2}% => [{3}]",
                res.Throughput, res.P95, res.ErrorRate, res.Passed ? "PASS" : "FAIL"));

            if (res.Passed) {
                lastGood = res;
                low = current;
                current *= 2; // Double concurrency
            } else {
                high = current;
                break;
            }
            Thread.Sleep(500); // Settle time between stages
        }

        if (high == 50 && (lastGood == null || !lastGood.Passed)) {
            Console.WriteLine("Baseline failed at initial concurrency 50.");
            return;
        }

        if (current > 32000 && lastGood != null && lastGood.Passed) {
            high = current;
        }

        // Step 2: Binary Search Refinement within [Low, High]
        Console.WriteLine("\n--- STAGE 2: BINARY SEARCH REFINEMENT ---");
        Console.WriteLine(string.Format("Searching bracket: [{0} .. {1}] with tolerance = 50", low, high));

        while ((high - low) > 50) {
            int mid = (low + high) / 2;
            Console.Write(string.Format("Testing Midpoint    = {0,4} ... ", mid));
            var res = TestConcurrency(baseUrl, mid, sampleSize);

            Console.WriteLine(string.Format("Throughput: {0,7:N0} RPS | P95: {1,6:N1} ms | Err: {2,4:N2}% => [{3}]",
                res.Throughput, res.P95, res.ErrorRate, res.Passed ? "PASS" : "FAIL"));

            if (res.Passed) {
                lastGood = res;
                low = mid; // Push higher
            } else {
                high = mid; // Back off
            }
            Thread.Sleep(500);
        }

        Console.WriteLine("\n============================================================");
        Console.WriteLine("FINAL VERDICT: MAXIMUM CONCURRENCY UPPER BOUND DISCOVERED");
        Console.WriteLine("============================================================");
        if (lastGood != null) {
            Console.WriteLine(string.Format("  Upper Bound Concurrency:  {0} concurrent users", lastGood.Concurrency));
            Console.WriteLine(string.Format("  Peak Throughput:          {0:N2} requests/sec", lastGood.Throughput));
            Console.WriteLine(string.Format("  Sustained P95 Latency:    {0:N2} ms (Budget: < 1,000 ms)", lastGood.P95));
            Console.WriteLine(string.Format("  Sustained P50 Latency:    {0:N2} ms", lastGood.P50));
            Console.WriteLine(string.Format("  Error Rate:               {0:N2}% (Budget: < 1.00%)", lastGood.ErrorRate));
        }
        Console.WriteLine("============================================================");
    }
}
"@

Add-Type -ReferencedAssemblies "System.Net.Http" -TypeDefinition $code -Language CSharp
[ConcurrencyFinder]::FindCeiling($Base, $SamplesPerStage)
