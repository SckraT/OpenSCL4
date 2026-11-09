//! The shape of a curve through values. The equalizer chart and the small profile curves draw with the same path,
//! so they interpolate the same way.

/// A path through `points`, as Slint path commands, smoothed so it follows the values instead of stepping between
/// them.
///
/// The slope at each point comes from the two neighbouring slopes (Fritsch–Carlson): zero at a turning point, and a
/// harmonic mean of the two slopes elsewhere, which keeps a rising stretch rising. The harmonic mean matters — a
/// plain Catmull-Rom spline overshoots, drawing a dip or a peak the device never has, which on an equalizer would
/// read as a band set to a value it is not.
pub fn smooth_path(points: &[(f32, f32)]) -> String {
    let Some(first) = points.first() else {
        return String::new();
    };
    let mut path = format!("M {:.1} {:.1}", first.0, first.1);
    path.push_str(&smooth_segments(points));
    path
}

/// The same curve as the `C` commands that follow its `M`, so a caller can walk it from a start point of its own
/// choosing: an area under the curve is closed at its corners rather than on the curve, and its top edge stays the
/// walk the line is stroked along. Empty when there is nothing to join.
pub fn smooth_segments(points: &[(f32, f32)]) -> String {
    let mut segments = String::new();
    if points.len() < 2 {
        return segments;
    }
    let slopes = slopes(points);
    for (index, pair) in points.windows(2).enumerate() {
        let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
        let dx = x1 - x0;
        // The control points sit a third of the way along the segment, on the tangent of the point they belong to.
        let control0 = (x0 + dx / 3.0, y0 + slopes[index] * dx / 3.0);
        let control1 = (x1 - dx / 3.0, y1 - slopes[index + 1] * dx / 3.0);
        segments.push_str(&format!(
            " C {:.1} {:.1} {:.1} {:.1} {:.1} {:.1}",
            control0.0, control0.1, control1.0, control1.1, x1, y1,
        ));
    }
    segments
}

/// The slope of the curve at each point, one per point.
fn slopes(points: &[(f32, f32)]) -> Vec<f32> {
    let between: Vec<f32> = points
        .windows(2)
        .map(|pair| {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            if x1 > x0 { (y1 - y0) / (x1 - x0) } else { 0.0 }
        })
        .collect();
    let last = between.len() - 1;
    let mut slopes = Vec::with_capacity(points.len());
    // The ends keep the slope of their only neighbouring segment.
    slopes.push(between[0]);
    for index in 1..between.len() {
        let (before, after) = (between[index - 1], between[index]);
        slopes.push(if before * after <= 0.0 {
            0.0
        } else {
            2.0 * before * after / (before + after)
        });
    }
    slopes.push(between[last]);
    slopes
}

#[cfg(test)]
mod tests {
    use super::{smooth_path, smooth_segments};

    /// Every walk over the curve, as (from, to, control0, control1).
    fn segments(path: &str) -> Vec<[f32; 6]> {
        let numbers: Vec<f32> = path
            .split(|c: char| c == 'M' || c == 'C' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .map(|part| part.parse().expect("a number"))
            .collect();
        segments_from_numbers(&numbers)
    }

    /// The first pair is the start point; each following six numbers are one segment.
    fn segments_from_numbers(numbers: &[f32]) -> Vec<[f32; 6]> {
        let mut start = (numbers[0], numbers[1]);
        let mut segments = Vec::new();
        for chunk in numbers[2..].chunks(6) {
            let end = (chunk[4], chunk[5]);
            segments.push([start.0, start.1, chunk[0], chunk[1], chunk[2], chunk[3]]);
            start = end;
        }
        segments
    }

    #[test]
    fn a_two_point_curve_is_the_straight_line_between_them() {
        let path = smooth_path(&[(0.0, 0.0), (30.0, 60.0)]);
        // Slope 2, so the controls sit a third along at a third of the rise.
        assert_eq!("M 0.0 0.0 C 10.0 20.0 20.0 40.0 30.0 60.0", path);
    }

    #[test]
    fn a_curve_never_reaches_past_the_values_it_joins() {
        // A rise onto a plateau: a spline with fitted slopes would bulge above the plateau here.
        let path = smooth_path(&[(0.0, 100.0), (10.0, 50.0), (20.0, 50.0), (30.0, 50.0)]);
        assert!(
            segments(&path)
                .iter()
                .all(|s| { s[3] >= 50.0 && s[5] >= 50.0 }),
            "{path}"
        );
    }

    #[test]
    fn a_turning_point_is_flat_so_the_curve_does_not_bulge_past_it() {
        let path = smooth_path(&[(0.0, 100.0), (10.0, 20.0), (20.0, 100.0)]);
        // The slope through the peak is zero, so both controls at the peak keep its height.
        assert!(path.contains("C 3.3 73.3 6.7 20.0 10.0 20.0"), "{path}");
        assert!(path.contains("C 13.3 20.0 16.7 73.3 20.0 100.0"), "{path}");
    }

    #[test]
    fn nothing_to_draw_has_no_path() {
        assert_eq!("", smooth_path(&[]));
        assert_eq!("M 5.0 5.0", smooth_path(&[(5.0, 5.0)]));
        assert_eq!("", smooth_segments(&[]));
        assert_eq!("", smooth_segments(&[(5.0, 5.0)]));
    }

    #[test]
    fn the_segments_are_the_walk_without_the_move_that_starts_it() {
        let points = [(0.0, 100.0), (10.0, 100.0), (20.0, 50.0)];
        let path = smooth_path(&points);
        assert_eq!(format!("M 0.0 100.0{}", smooth_segments(&points)), path);
        // A flat run starts with a straight segment, so the walk can be joined from either end.
        assert!(
            smooth_segments(&points).starts_with(" C 3.3 100.0 6.7 100.0"),
            "{}",
            smooth_segments(&points)
        );
    }
}
