use std::{
    sync::mpsc::{channel, Receiver},
    time::Duration,
};

use knyst::{
    controller::Controller,
    graph::RunGraph,
    prelude::{Graph, GraphSettings, KnystCommands, RunGraphSettings},
    KnystError, Resources, ResourcesSettings,
};

pub struct BelaKnystBackend {
    run_graph: RunGraph,
    controller: Controller,
    unhandled_errors: Receiver<KnystError>,
}

impl BelaKnystBackend {
    /// Creates a new BelaKnystBackend as well as a top level graph
    pub fn new(sample_rate: f32, block_size: usize, num_inputs: usize, num_outputs: usize) -> Self {
        let resources = Resources::new(ResourcesSettings::default());
        let mut graph = Graph::new(GraphSettings {
            num_inputs,
            num_outputs,
            block_size,
            sample_rate,
            max_node_inputs: 12,
            ..Default::default()
        });
        let run_graph_settings = RunGraphSettings {
            scheduling_latency: Duration::ZERO,
        };
        let (run_graph, resources_command_sender, resources_command_receiver) =
            RunGraph::new(&mut graph, resources, run_graph_settings).expect("Since we are creating the Graph here there should be no reason for RunGraph::new to fail");
        let (error_producer, error_consumer) = channel();
        let error_handler = move |err| {
            error_producer.send(err).expect("The Receiver should be dropped after anything that can send errors via this callback");
        };
        let controller = Controller::new(
            graph,
            error_handler,
            resources_command_sender,
            resources_command_receiver,
        );
        Self {
            run_graph,
            controller,
            unhandled_errors: error_consumer,
        }
    }
    /// Get a KnystCommands. This way, by passing a pointer to the
    /// BelaKnystBackend to your Rust sounds synthesis setup code, that code can
    /// get access to a KnystCommands.
    pub fn knyst_commands(&mut self) -> KnystCommands {
        self.controller.get_knyst_commands()
    }
    /// Run updates for Controller and RunGraph. Non realtime thread safe
    pub fn update(&mut self) {
        self.controller.run(500);
    }
    /// Sets the input of one channel to the graph. Run before `process_block`
    pub fn set_input_channel(&mut self, channel_index: usize, input_channel: &[f32]) {
        let graph_input_buffers = self.run_graph.graph_input_buffers();
        assert_eq!(graph_input_buffers.block_size(), input_channel.len());
        for i in 0..graph_input_buffers.block_size() {
            graph_input_buffers.write(input_channel[i], channel_index, i);
        }
    }
    pub fn process_block(&mut self) {
        self.run_graph.run_resources_communication(50);
        self.run_graph.process_block();
    }
    /// Gets the output of one channel from the graph. Run after `process_block`
    pub fn get_output_channel(&self, index: usize) -> &[f32] {
        self.run_graph.graph_output_buffers().get_channel(index)
    }
    pub fn next_error(&mut self) -> Option<KnystError> {
        self.unhandled_errors.try_recv().ok()
    }
}

/// Creates an opaque pointer to the BelaKnystBackend
///
/// # Safety
/// Make sure you destroy the SynthesisInterface with `bela_knyst_backend_destroy`
/// once you are done with it.
#[no_mangle]
pub unsafe extern "C" fn bela_knyst_backend_create(
    sample_rate: libc::c_float,
    block_size: libc::size_t,
    num_inputs: libc::size_t,
    num_outputs: libc::size_t,
) -> *mut BelaKnystBackend {
    Box::into_raw(Box::new(BelaKnystBackend::new(
        sample_rate as f32,
        block_size,
        num_inputs,
        num_outputs,
    )))
}
/// Run updates (non realtime thread safe)
///
/// # Safety
/// Only call with a pointer received from `bela_knyst_backend_create`
#[no_mangle]
pub unsafe extern "C" fn bela_knyst_backend_update(bela_knyst_backend_ptr: *mut BelaKnystBackend) {
    if !bela_knyst_backend_ptr.is_null() {
        (*bela_knyst_backend_ptr).update();
    }
}
/// Set an input channel. Run before process_block
///
/// # Safety
/// Only call with a pointer received from `bela_knyst_backend_create`
#[no_mangle]
pub unsafe extern "C" fn bela_knyst_backend_set_input_channel(
    bela_knyst_backend_ptr: *mut BelaKnystBackend,
    channel_index: libc::size_t,
    input_channel_ptr: *const libc::c_float,
    block_size: libc::size_t,
) {
    if !bela_knyst_backend_ptr.is_null() {
        let input_channel = std::slice::from_raw_parts(input_channel_ptr as *const f32, block_size);
        (*bela_knyst_backend_ptr).set_input_channel(channel_index, input_channel);
    }
}
/// Process one block of audio
///
/// # Safety
/// Only call with a pointer received from `bela_knyst_backend_create`
#[no_mangle]
pub unsafe extern "C" fn bela_knyst_backend_process_block(
    bela_knyst_backend_ptr: *mut BelaKnystBackend,
) {
    if !bela_knyst_backend_ptr.is_null() {
        (*bela_knyst_backend_ptr).process_block();
    }
}
/// Get an output channel. Run after process_block
///
/// # Safety
/// Only call with a pointer received from `bela_knyst_backend_create`. Use immediately after receiving it and then never use the pointer again.
#[no_mangle]
pub unsafe extern "C" fn bela_knyst_backend_get_output_channel(
    bela_knyst_backend_ptr: *mut BelaKnystBackend,
    channel_index: libc::size_t,
) -> *const libc::c_float {
    if !bela_knyst_backend_ptr.is_null() {
        let output_channel = (*bela_knyst_backend_ptr).get_output_channel(channel_index);
        output_channel.as_ptr()
    } else {
        std::ptr::null()
    }
}

/// Drops the BelaKnystBackend
/// # Safety
/// This will drop a BelaKnystBackend if the pointer is not null. Don't give it anything
/// other than a pointer to a BelaKnystBackend gotten from `bela_knyst_backend_create`.
#[no_mangle]
pub unsafe extern "C" fn bela_knyst_backend_destroy(bela_knyst_backend_ptr: *mut BelaKnystBackend) {
    if !bela_knyst_backend_ptr.is_null() {
        drop(Box::from_raw(bela_knyst_backend_ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {}
}
