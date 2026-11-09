// Package edgeflag provides zero-overhead Linux kernel bypass abstractions.
package edgeflag

import "sync"

// PacketDescriptor represents a single zero-copy UMEM network frame.
type PacketDescriptor struct {
	Addr  uint64
	Len   uint32
	Flags uint32
}

// UmemPool manages contiguous user-space memory frames for AF_XDP simulation.
type UmemPool struct {
	mu         sync.Mutex
	buffer     []byte
	frameSize  int
	numFrames  int
	freeFrames []uint64
}

// NewUmemPool initializes a UMEM contiguous frame ring buffer.
func NewUmemPool(numFrames, frameSize int) *UmemPool {
	free := make([]uint64, numFrames)
	for i := 0; i < numFrames; i++ {
		free[i] = uint64(i * frameSize)
	}

	return &UmemPool{
		buffer:     make([]byte, numFrames*frameSize),
		frameSize:  frameSize,
		numFrames:  numFrames,
		freeFrames: free,
	}
}

// AllocFrame allocates a zero-copy frame address.
func (p *UmemPool) AllocFrame() (uint64, bool) {
	p.mu.Lock()
	defer p.mu.Unlock()
	if len(p.freeFrames) == 0 {
		return 0, false
	}
	addr := p.freeFrames[len(p.freeFrames)-1]
	p.freeFrames = p.freeFrames[:len(p.freeFrames)-1]
	return addr, true
}

// FreeFrame recycles a frame address into the pool.
func (p *UmemPool) FreeFrame(addr uint64) {
	p.mu.Lock()
	defer p.mu.Unlock()
	p.freeFrames = append(p.freeFrames, addr)
}

// GetSlice returns a zero-copy slice view into the frame payload.
func (p *UmemPool) GetSlice(desc PacketDescriptor) []byte {
	return p.buffer[desc.Addr : desc.Addr+uint64(desc.Len)]
}
