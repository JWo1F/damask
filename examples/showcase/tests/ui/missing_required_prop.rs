//! `<Notice detail={None}/>` — `title` has no `Option` to say it may be left
//! out, so the builder never reaches `__damask_build`.

use damask_showcase::notice::Notice;

fn main() {
    let _ = Notice::__damask_props().detail(None).__damask_build();
}
