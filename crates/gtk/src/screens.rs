//! Every screen shows the installer. On the live system each screen shows
//! the same part of cage's layout (nix/live/mirror-screens.sh lays them on
//! top of each other, centred); screens of different shapes show a little
//! more of it on two sides than the others, so the installer keeps its
//! content inside the part every screen shows and leaves the rest as
//! background: nothing is cut off on any of them.

use std::cell::Cell;
use std::rc::Rc;

use gtk::gdk;
use gtk::prelude::*;

/// A screen's or an area's place in the layout: x, y, width, height.
type Rect = (i32, i32, i32, i32);

/// How far the part every screen shows lies inside the whole layout:
/// left, top, right, bottom. Nothing with one screen, or when the screens
/// don't overlap (side by side: not mirrored).
fn insets(screens: &[Rect]) -> (i32, i32, i32, i32) {
    if screens.len() < 2 {
        return (0, 0, 0, 0);
    }
    let (mut ux, mut uy, mut ur, mut ub) = (i32::MAX, i32::MAX, i32::MIN, i32::MIN);
    let (mut ix, mut iy, mut ir, mut ib) = (i32::MIN, i32::MIN, i32::MAX, i32::MAX);
    for &(x, y, w, h) in screens {
        (ux, uy, ur, ub) = (ux.min(x), uy.min(y), ur.max(x + w), ub.max(y + h));
        (ix, iy, ir, ib) = (ix.max(x), iy.max(y), ir.min(x + w), ib.min(y + h));
    }
    if ix >= ir || iy >= ib {
        return (0, 0, 0, 0);
    }
    (ix - ux, iy - uy, ur - ir, ub - ib)
}

thread_local! {
    /// The insets `fit_every_screen` keeps to now.
    static INSETS: Cell<(i32, i32, i32, i32)> = const { Cell::new((0, 0, 0, 0)) };
}

/// For other windows that fill the layout (the terminal): the padding on
/// each side, horizontal and vertical, that keeps them inside what every
/// screen shows; `None` when they needn't keep in.
pub fn padding() -> Option<(i32, i32)> {
    let (left, top, right, bottom) = INSETS.get();
    let padding = (left.max(right), top.max(bottom));
    (padding != (0, 0)).then_some(padding)
}

/// Keeps `content` inside the part of the layout every screen shows, as
/// screens come, go and change.
pub fn fit_every_screen(content: &impl IsA<gtk::Widget>) {
    let Some(display) = gdk::Display::default() else {
        return;
    };
    let content = content.as_ref().clone();
    let monitors = display.monitors();
    let update: Rc<dyn Fn()> = {
        let monitors = monitors.clone();
        Rc::new(move || {
            let screens: Vec<Rect> = (0..monitors.n_items())
                .filter_map(|i| monitors.item(i)?.downcast::<gdk::Monitor>().ok())
                .map(|m| {
                    let g = m.geometry();
                    (g.x(), g.y(), g.width(), g.height())
                })
                .collect();
            let (mut left, top, mut right, bottom) = insets(&screens);
            INSETS.set((left, top, right, bottom));
            if content.direction() == gtk::TextDirection::Rtl {
                std::mem::swap(&mut left, &mut right);
            }
            content.set_margin_start(left);
            content.set_margin_top(top);
            content.set_margin_end(right);
            content.set_margin_bottom(bottom);
        })
    };
    let watch = {
        let update = update.clone();
        move |monitor: gdk::Monitor| {
            let update = update.clone();
            monitor.connect_geometry_notify(move |_| update());
        }
    };
    for i in 0..monitors.n_items() {
        if let Some(m) = monitors.item(i).and_then(|m| m.downcast().ok()) {
            watch(m);
        }
    }
    {
        let update = update.clone();
        monitors.connect_items_changed(move |list, position, _removed, added| {
            for i in position..position + added {
                if let Some(m) = list.item(i).and_then(|m| m.downcast().ok()) {
                    watch(m);
                }
            }
            update();
        });
    }
    update();
}

#[cfg(test)]
mod tests {
    use super::insets;

    #[test]
    fn one_screen_or_side_by_side_fill_the_window() {
        assert_eq!(insets(&[(0, 0, 1920, 1080)]), (0, 0, 0, 0));
        assert_eq!(
            insets(&[(0, 0, 1920, 1080), (1920, 0, 1920, 1080)]),
            (0, 0, 0, 0)
        );
    }

    #[test]
    fn mirrored_screens_keep_to_the_common_part() {
        // Same shape: all of it.
        assert_eq!(
            insets(&[(0, 0, 1920, 1080), (0, 0, 1920, 1080)]),
            (0, 0, 0, 0)
        );
        // 16:10 (2880x1800 at 1.5) over 16:9, centred: 60 above and below.
        assert_eq!(
            insets(&[(0, 0, 1920, 1200), (0, 60, 1920, 1080)]),
            (0, 60, 0, 60)
        );
    }
}
