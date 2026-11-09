/**
 * Zero-Overhead Singleflight Request Coalescing Feature for TypeScript.
 */

export class Singleflight<T> {
  private inFlight = new Map<string, Promise<T>>();

  public async execute(key: string, fn: () => Promise<T>): Promise<T> {
    const existing = this.inFlight.get(key);
    if (existing) {
      return existing;
    }

    const promise = fn().finally(() => {
      this.inFlight.delete(key);
    });

    this.inFlight.set(key, promise);
    return promise;
  }
}
