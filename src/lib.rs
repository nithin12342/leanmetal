//! EdgeFlag Daemon Library Root
//! High-Throughput, Low-Latency Feature Flag Engine

pub mod domain;
pub mod interfaces;

pub use domain::engine::{
    compute_bucket, evaluate_flag, from_slice_scratch, from_slice_simd, is_in_rollout, to_vec_simd,
    AttributeValue, ComparisonOp, Condition, EdgeFlagGuard, EdgeFlagLayer, EvaluationContext,
    EvaluationResult, FlagDefinition, FlagGuard, OpenFeatureResolution, SimdJsonError, Singleflight,
    TargetingRule,
};

pub use domain::ingress::{
    AfXdpConfig, AfXdpError, BackpressureHub, ClientGuard, ClientSession, ConnectionRing,
    KernelBypassEngine, L1StaticCache, MokaL1Cache, NativeXskEngine, PacketDescriptor, PushResult,
    UmemPool, XdpDescriptorRing, XdpMode, XdpStats,
};
pub use domain::invalidation::{
    GossipMeshBus, InvalidationError, InvalidationMessage, ReplicationDelta, ValkeyMeshBus,
    INVALIDATION_CHANNEL,
};
pub use domain::social::{
    CreatePostRequest, EnrichedPost, MmapSocialStore, PostRecord, SocialEngine,
    SocialEngineError, SocialStorageError, SocialStore, TimelineQuery, TimelineResponse, UserProfile,
};
pub use domain::storage::{
    AsyncWal, AuditEntry, AuditWal, MmapFlagStore, StorageError, WalError,
};
pub use interfaces::http::{
    build_benchmark_router, build_router, AppState, DaemonMetrics, EvaluateRequest, EvaluateResponse,
};
pub use interfaces::proxy::{EdgeProxyConfig, EdgeProxyStatus};
