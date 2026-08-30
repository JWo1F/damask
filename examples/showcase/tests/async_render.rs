//! End-to-end behavior of the async example components: a template whose own
//! `.dmk` contains `.await` renders through `AsyncComponent::render_async`
//! rather than `Component::render`, composes with a plain sync child, and
//! still supports snippets and `<slot>` fallbacks.

use damask::AsyncComponent;
use damask_showcase::async_greeting::AsyncGreeting;
use damask_showcase::async_panel::AsyncPanel;

/// A single-threaded, no-IO executor: no runtime dependency, and every future
/// here resolves without ever really suspending, so busy-polling with a
/// no-op waker is enough.
fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
    }
}

#[test]
fn a_direct_await_renders_through_render_async() {
    let g = AsyncGreeting {
        name: "<Ada>".into(),
    };
    assert_eq!(block_on(g.render_async()), "Hello &lt;Ada&gt;!");
}

#[test]
fn an_async_template_composes_a_sync_child_a_snippet_and_a_slot_fallback() {
    let p = AsyncPanel { label: "ok".into() };
    let out = block_on(p.render_async());
    assert!(out.contains("<button>fixed</button>"), "{out}");
    assert!(out.contains("<p>OK</p>"), "{out}");
    assert!(out.contains("<footer>ok</footer>"), "{out}");
}

/// The property the `Send` bounds on `RenderFuture`, `Renderer` and a slot's
/// content all exist for: a render can be awaited by an executor that steals
/// work between threads, which is every server executor. Without it an async
/// template could not be rendered from a request handler at all — so this is a
/// compile-time assertion first and a test second.
#[test]
fn a_render_future_can_be_awaited_on_a_work_stealing_executor() {
    fn assert_send<T: Send>(value: T) -> T {
        value
    }

    let p = AsyncPanel {
        label: "sendable".into(),
    };
    let out = block_on(assert_send(async move { p.render_async().await }));
    assert!(out.contains("<footer>sendable</footer>"), "{out}");
}

/// A caller's fill reaches an async template's `<slot>` — the path that holds
/// the `Slots` across the awaits either side of it.
#[test]
fn a_fill_reaches_an_async_slot() {
    use damask::{Render, Renderer, Slot, Slots};

    struct Footer;
    impl Render for Footer {
        fn render_into(&self, r: &mut dyn Renderer) {
            r.write_raw("filled");
        }
    }

    let p = AsyncPanel {
        label: "ignored".into(),
    };
    let footer = Footer;
    let entries = [Slot::new("footer", &footer)];
    let out = block_on(p.render_with_async(Slots::new(&entries)));
    assert!(out.contains("<footer>filled</footer>"), "{out}");
}
