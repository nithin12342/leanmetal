use engine_layout::EventEnvelope;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandIntent {
    pub aggregate_id: u64,
    pub expected_revision: u64,
    pub event_type: u16,
    pub origin_node_id: u32,
    pub payload: [u8; 16],
}

pub struct CommandProcessor;

impl CommandProcessor {
    pub fn process_command(intent: CommandIntent, event_id: u64, timestamp_ns: u64) -> Result<EventEnvelope, &'static str> {
        if intent.aggregate_id == 0 {
            return Err("Aggregate ID must not be zero");
        }

        let envelope = EventEnvelope {
            event_id,
            aggregate_id: intent.aggregate_id,
            revision: intent.expected_revision + 1,
            timestamp_ns,
            schema_version: 1,
            payload_len: 16,
            origin_node_id: intent.origin_node_id,
            event_type: intent.event_type,
            flags: 0,
            payload_inline: intent.payload,
        };

        Ok(envelope)
    }
}
