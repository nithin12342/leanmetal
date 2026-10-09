"""
Zero-Overhead Linux Kernel Bypass & AF_XDP Simulation Feature.
"""

from typing import List, Optional


class PacketDescriptor:
    def __init__(self, addr: int, length: int, flags: int = 0):
        self.addr = addr
        self.length = length
        self.flags = flags


class UmemPool:
    """Pre-allocated contiguous user-space memory pool (UMEM) for zero-copy DMA frames."""

    def __init__(self, num_frames: int, frame_size: int = 2048):
        self.frame_size = frame_size
        self.num_frames = num_frames
        self.buffer = bytearray(num_frames * frame_size)
        self.free_frames: List[int] = [i * frame_size for i in range(num_frames)]

    def alloc_frame(self) -> Optional[int]:
        return self.free_frames.pop() if self.free_frames else None

    def free_frame(self, addr: int) -> None:
        if addr + self.frame_size <= len(self.buffer):
            self.free_frames.append(addr)

    def get_slice(self, desc: PacketDescriptor) -> memoryview:
        start = desc.addr
        end = start + desc.length
        return memoryview(self.buffer)[start:end]
