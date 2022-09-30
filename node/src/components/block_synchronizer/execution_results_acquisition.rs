use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Debug},
};

use datasize::DataSize;
use derive_more::From;
use serde::Serialize;
use tracing::{debug, error, warn};

use casper_hashing::{ChunkWithProof, Digest};
use casper_types::bytesrepr::{self};

use crate::{
    components::{
        fetcher::{FetchResult, FetchedData},
        Component,
    },
    effect::{
        announcements::PeerBehaviorAnnouncement, requests::FetcherRequest, EffectBuilder,
        EffectExt, Effects, Responder,
    },
    types::{
        BlockExecutionResultsOrChunk, BlockExecutionResultsOrChunkId, BlockHash, Item, NodeId,
        ValueOrChunk,
    },
    NodeRng,
};

#[derive(Clone, Copy, PartialEq, Eq, DataSize, Debug)]
pub(crate) enum ExecutionResultsRootHash {
    Legacy,
    Some(Digest),
}

impl ExecutionResultsRootHash {
    fn is_legacy(&self) -> bool {
        matches!(self, ExecutionResultsRootHash::Legacy)
    }
}

#[derive(Clone, PartialEq, Eq, DataSize, Debug)]
pub(crate) struct ExecutionResultsAcquisition {
    block_hash: BlockHash,
    results_root_hash: ExecutionResultsRootHash,
    chunks: HashMap<u64, ChunkWithProof>,
    chunk_count: Option<u64>,
}

impl ExecutionResultsAcquisition {
    pub(crate) fn new(block_hash: BlockHash, results_root_hash: ExecutionResultsRootHash) -> Self {
        ExecutionResultsAcquisition {
            block_hash,
            results_root_hash,
            chunks: HashMap::new(),
            chunk_count: None,
        }
    }

    pub(super) fn apply_block_execution_results_or_chunk(
        &mut self,
        block_execution_results_or_chunk: BlockExecutionResultsOrChunk,
    ) -> Option<Vec<casper_types::ExecutionResult>> {
        debug!(%block_execution_results_or_chunk, "got block execution results or chunk");
        let (block_hash, value) = match block_execution_results_or_chunk {
            BlockExecutionResultsOrChunk::Legacy { block_hash, value } => (block_hash, value),
            BlockExecutionResultsOrChunk::Contemporary { block_hash, value } => (block_hash, value),
        };

        // TODO - reinstate?
        // if self.block_hash != block_hash {
        //     return Outcome::Got {
        //         value: Err(Error::BlockHashMismatch {
        //             expected: self.block_hash,
        //             provided: block_hash,
        //         }),
        //     };
        // }

        match value {
            ValueOrChunk::Value(execution_results) => {
                debug!(%block_hash, "got a full set of execution results (unchunked)");
                // TODO - check merkle root
                Some(execution_results)
            }
            ValueOrChunk::ChunkWithProof(chunk) => self.consume_chunk(chunk),
        }
    }

    fn consume_chunk(
        &mut self,
        chunk: ChunkWithProof,
    ) -> Option<Vec<casper_types::ExecutionResult>> {
        let digest = chunk.proof().root_hash();
        let index = chunk.proof().index();
        let count = chunk.proof().count();
        match self.chunk_count {
            None => {
                self.chunk_count = Some(count);
            }
            Some(existing_count) => {
                if existing_count != count {
                    debug!(existing_count, ?chunk, "chunk with different count");
                    return None;
                }
            }
        }

        // Add the downloaded chunk to cache.
        match self.results_root_hash {
            ExecutionResultsRootHash::Legacy => {
                // TODO - we just accept the first chunk's digest -
                //        we need to store all chunks regardless of the root hash
                //        provided in each chunk.
                // self.results_root_hash = Some(digest);
            }
            ExecutionResultsRootHash::Some(root_hash) => {
                if root_hash != digest {
                    debug!(%root_hash, ?chunk, "chunk with different root hash");
                    return None;
                }
            }
        }
        let _ = self.chunks.insert(index, chunk);
        self.assemble_chunks()
    }

    pub(super) fn needs_value_or_chunk(&self) -> Option<BlockExecutionResultsOrChunkId> {
        let id = if self.results_root_hash.is_legacy() {
            BlockExecutionResultsOrChunkId::legacy(self.block_hash)
        } else {
            BlockExecutionResultsOrChunkId::new(self.block_hash)
        };

        self.missing_chunk()
            .map(|missing_index| id.next_chunk(missing_index))
    }

    fn missing_chunk(&self) -> Option<u64> {
        let count = self.chunk_count.unwrap_or_default();
        (0..count).find(|idx| !self.chunks.contains_key(idx))
    }

    fn assemble_chunks(&mut self) -> Option<Vec<casper_types::ExecutionResult>> {
        let count = match self.chunk_count {
            Some(count) => count,
            None => return None,
        };
        let serialized: Vec<u8> = (0..count)
            .filter_map(|index| self.chunks.get(&index))
            .flat_map(|chunk| chunk.chunk())
            .copied()
            .collect();
        match bytesrepr::deserialize(serialized) {
            Ok(value) => {
                // TODO - check merkle root
                Some(value)
            }
            Err(error) => {
                error!(%error, "failed to deserialize execution results");
                self.chunks.clear();
                self.chunk_count = None;
                None
            }
        }
    }
}
