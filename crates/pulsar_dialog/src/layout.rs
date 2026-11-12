//! How big a dialog's window is. A window cannot size itself to its content
//! before it opens, so the height is estimated from the text.

use pulsar_dialog_protocol::DialogRequest;

/// The window's width, in logical pixels.
pub const WIDTH: f32 = 520.0;
const MIN_HEIGHT: f32 = 190.0;
const MAX_HEIGHT: f32 = 620.0;
/// Characters that fit on one line of the message at [`WIDTH`].
const MESSAGE_COLUMNS: usize = 54;
const DETAIL_COLUMNS: usize = 66;

/// The lines `text` takes when wrapped at `columns`, counting its own breaks.
pub fn wrapped_lines(text: &str, columns: usize) -> usize {
    text.split('\n')
        .map(|line| line.chars().count().max(1).div_ceil(columns))
        .sum()
}

/// The window height for a request.
pub fn height(request: &DialogRequest) -> f32 {
    // Title bar, padding and the button row.
    let mut height = 32.0 + 40.0 + 60.0;
    height += wrapped_lines(&request.message, MESSAGE_COLUMNS) as f32 * 22.0;
    if let Some(detail) = &request.detail {
        height += 12.0 + wrapped_lines(detail, DETAIL_COLUMNS) as f32 * 18.0;
    }
    if request.checkbox.is_some() {
        height += 36.0;
    }
    height.clamp(MIN_HEIGHT, MAX_HEIGHT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_wrap_at_the_column_count_and_keep_their_own_breaks() {
        assert_eq!(wrapped_lines("short", 54), 1);
        assert_eq!(wrapped_lines(&"x".repeat(55), 54), 2);
        assert_eq!(wrapped_lines("a\n\nb", 54), 3);
        assert_eq!(wrapped_lines("", 54), 1);
    }

    #[test]
    fn a_dialog_grows_with_its_text_within_limits() {
        let short = height(&DialogRequest::new("t", "Continue?"));
        let long = height(&DialogRequest::new("t", "line\n".repeat(8)).detail("d".repeat(300)));
        assert!(short >= MIN_HEIGHT && long > short);
        assert_eq!(height(&DialogRequest::new("t", "x\n".repeat(500))), MAX_HEIGHT);
        // Past the minimum height, where the clamp no longer hides the difference.
        let tall = "line\n".repeat(6);
        assert!(
            height(&DialogRequest::new("t", tall.clone()).checkbox("c", false))
                > height(&DialogRequest::new("t", tall))
        );
    }
}
