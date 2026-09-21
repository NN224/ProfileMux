use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::tui::dialogs::{ConfirmationDialog, DeleteInfo};

/// Computes modal dialog height based on content line count, clamped to available height.
/// Rows a logical line occupies once wrapped to `inner_width`.
pub fn wrapped_rows(line_width: usize, inner_width: usize) -> usize {
    if inner_width == 0 {
        return 1;
    }
    line_width.div_ceil(inner_width).max(1)
}

/// Total rows `lines` occupy once wrapped to `inner_width`.
pub fn wrapped_line_count(lines: &[Line<'static>], inner_width: usize) -> usize {
    lines
        .iter()
        .map(|l| wrapped_rows(l.width(), inner_width))
        .sum()
}

pub fn compute_dialog_height(content_lines: usize, available_height: u16) -> u16 {
    let desired = (content_lines as u16).saturating_add(3).max(4);
    let max_h = available_height.saturating_sub(2);
    desired.min(max_h)
}

/// Computes modal dialog width based on maximum line length, clamped to terminal bounds.
pub fn compute_dialog_width(max_line_len: usize, available_width: u16) -> u16 {
    let desired = (max_line_len as u16).saturating_add(4).clamp(72, 100);
    let max_w = available_width.saturating_sub(4);
    desired.min(max_w)
}

/// Clamps scroll offset so it cannot exceed available content lines or underflow.
pub fn clamp_scroll(scroll: usize, total_lines: usize, visible_height: usize) -> usize {
    if total_lines <= visible_height || visible_height == 0 {
        0
    } else {
        let max_scroll = total_lines.saturating_sub(visible_height);
        scroll.min(max_scroll)
    }
}

/// Slices rendered lines according to scroll offset, adding an indicator when more exists.
/// The slice of `lines` that fits in `visible_height` rows once wrapped to
/// `inner_width`, with a trailing indicator when content is scrolled out of
/// view. Wrapping is accounted for so a long path can never push the last line
/// off the dialog silently.
pub fn visible_lines(
    lines: &[Line<'static>],
    scroll: usize,
    visible_height: usize,
    inner_width: usize,
) -> Vec<Line<'static>> {
    if visible_height == 0 || lines.is_empty() {
        return Vec::new();
    }
    let total_rows = wrapped_line_count(lines, inner_width);
    if total_rows <= visible_height {
        return lines.to_vec();
    }

    let start = clamp_scroll(scroll, lines.len(), visible_height);
    let tail = &lines[start..];

    if wrapped_line_count(tail, inner_width) <= visible_height {
        return tail.to_vec();
    }

    let budget = visible_height.saturating_sub(1);
    let mut used = 0usize;
    let mut out = Vec::new();
    for line in tail {
        let rows = wrapped_rows(line.width(), inner_width);
        if used + rows > budget {
            break;
        }
        used += rows;
        out.push(line.clone());
    }
    out.push(Line::styled(
        "  ... more",
        Style::default().fg(Color::DarkGray),
    ));
    out
}

fn extract_size_str(bytes_opt: Option<u64>, str_val: &str) -> Option<String> {
    if let Some(b) = bytes_opt {
        if b > 0 {
            return Some(crate::fs::size::format_bytes(b));
        }
    }
    if !str_val.is_empty() && str_val != "not measured" && str_val != "0 B" {
        Some(str_val.to_string())
    } else {
        None
    }
}

/// Builds prominent storage breakdown lines for deletion confirmation.
pub fn build_delete_size_lines(
    info: Option<&DeleteInfo>,
    reclaimed_bytes: Option<u64>,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let core_val = info.and_then(|i| extract_size_str(i.core_bytes, &i.profile_data_size));
    let cache_val = info.and_then(|i| extract_size_str(i.cache_bytes, &i.cache_size));
    let total_val = info
        .and_then(|i| extract_size_str(i.total_bytes, &i.total_size))
        .or_else(|| reclaimed_bytes.and_then(|b| extract_size_str(Some(b), "")));

    if let Some(c) = core_val {
        lines.push(Line::from(vec![
            Span::styled(
                "    Profile data:  ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(c),
        ]));
    }
    if let Some(ca) = cache_val {
        lines.push(Line::from(vec![
            Span::styled(
                "    Cache:         ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(ca),
        ]));
    }
    if let Some(t) = total_val {
        lines.push(Line::from(vec![
            Span::styled(
                "    Total:         ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(t),
        ]));
    }
    lines
}

/// Formats delete info into formatted lines.
pub fn build_delete_info_lines(info: &DeleteInfo) -> Vec<Line<'static>> {
    build_delete_size_lines(Some(info), None)
}

/// Constructs the full rendered body for a confirmation dialog.
pub fn build_confirmation_lines(dialog: &ConfirmationDialog) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let plan_lines = dialog.plan.lines();
    let size_lines = if dialog.plan.kind == crate::domain::OperationKind::DeleteProfile
        || dialog.delete_info.is_some()
    {
        build_delete_size_lines(dialog.delete_info.as_ref(), dialog.plan.reclaimed_bytes)
    } else {
        Vec::new()
    };

    if size_lines.is_empty() {
        for l in plan_lines {
            lines.push(Line::raw(l));
        }
        return lines;
    }

    let steps_pos = plan_lines.iter().position(|l| l == "Steps:");
    if let Some(pos) = steps_pos {
        for l in &plan_lines[..pos] {
            lines.push(Line::raw(l.clone()));
        }
        for sl in size_lines {
            lines.push(sl);
        }
        lines.push(Line::raw(""));
        for l in &plan_lines[pos..] {
            lines.push(Line::raw(l.clone()));
        }
    } else {
        for l in plan_lines {
            lines.push(Line::raw(l));
        }
        lines.push(Line::raw(""));
        for sl in size_lines {
            lines.push(sl);
        }
    }

    lines
}
