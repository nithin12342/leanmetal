/**
 * Zero-Overhead Kernel Bypass & UMEM Frame Buffer Simulator for Node.js / TypeScript.
 */

export interface PacketDescriptor {
  addr: number;
  length: number;
  flags?: number;
}

export class UmemPool {
  private buffer: Uint8Array;
  private freeFrames: number[] = [];
  public readonly frameSize: number;

  constructor(numFrames: number, frameSize: number = 2048) {
    this.frameSize = frameSize;
    this.buffer = new Uint8Array(numFrames * frameSize);
    for (let i = 0; i < numFrames; i++) {
      this.freeFrames.push(i * frameSize);
    }
  }

  public allocFrame(): number | undefined {
    return this.freeFrames.pop();
  }

  public freeFrame(addr: number): void {
    if (addr + this.frameSize <= this.buffer.length) {
      this.freeFrames.push(addr);
    }
  }

  public getSlice(desc: PacketDescriptor): Uint8Array {
    return this.buffer.subarray(desc.addr, desc.addr + desc.length);
  }
}
