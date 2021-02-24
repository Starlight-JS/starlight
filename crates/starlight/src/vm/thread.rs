use wtf_rs::stack_bounds::StackBounds;

pub struct Thread {
    pub bounds: StackBounds,
}

thread_local! {
    pub static THREAD: Thread = {
        let bounds = StackBounds::current_thread_stack_bounds();
        Thread {
            bounds
        }
    }
}
