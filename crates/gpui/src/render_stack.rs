// Recursive div layout already checks its stack, but a Render implementation
// can build a large, finite tree before layout reaches the next div. Debug
// toolbar builders alone exceed stacksafe's default 128 KiB margin. Keep the
// check outside that builder's frame, including for views nested inside layout.
#[inline(never)]
pub(crate) fn with_render_stack<R>(render: impl FnOnce() -> R) -> R {
    stacker::maybe_grow(512 * 1024, 2 * 1024 * 1024, render)
}

#[cfg(test)]
mod render_stack_tests {
    use super::with_render_stack;

    #[inline(never)]
    fn nested_render(depth: usize) -> usize {
        with_render_stack(|| {
            let mut frame = [0_u8; 256 * 1024];
            std::hint::black_box(&mut frame);
            frame[0] = 1;
            let nested = if depth == 0 {
                0
            } else {
                nested_render(depth - 1)
            };
            nested + usize::from(std::hint::black_box(&frame)[0])
        })
    }

    #[test]
    fn finite_render_frames_grow_on_a_small_stack() {
        std::thread::Builder::new()
            .stack_size(192 * 1024)
            .spawn(|| assert_eq!(nested_render(12), 13))
            .unwrap()
            .join()
            .unwrap();
    }
}
