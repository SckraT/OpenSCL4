//! Lists the window shows, changed in place. A new model makes Slint build the list's elements anew, which drops
//! what they keep themselves, such as a slider showing the value it sent while the device answers. When the list
//! keeps its length, its rows are changed one by one instead, and the elements stay.

use slint::{Model, ModelRc, VecModel};

use crate::SettingsPanel;

/// Puts the rows into the model the window shows when that model has as many, and gives a new model otherwise.
pub fn updated<T: Clone + PartialEq + 'static>(
    current: &ModelRc<T>,
    rows: Vec<T>,
) -> Option<ModelRc<T>> {
    let Some(model) = current
        .as_any()
        .downcast_ref::<VecModel<T>>()
        .filter(|model| model.row_count() == rows.len())
    else {
        return Some(ModelRc::new(VecModel::from(rows)));
    };
    for (index, row) in rows.into_iter().enumerate() {
        if model.row_data(index).as_ref() != Some(&row) {
            model.set_row_data(index, row);
        }
    }
    None
}

/// The panels of a section page, each with its rows changed in place where the panel stays of the same kind.
pub fn updated_panels(
    current: &ModelRc<SettingsPanel>,
    panels: Vec<SettingsPanel>,
) -> Option<ModelRc<SettingsPanel>> {
    let same_kinds = current.row_count() == panels.len()
        && panels.iter().enumerate().all(|(i, panel)| {
            current
                .row_data(i)
                .is_some_and(|old| old.kind == panel.kind)
        });
    if !same_kinds {
        return updated(current, panels);
    }
    // keep each panel's own list of rows, so that its row elements stay too
    let panels = panels
        .into_iter()
        .enumerate()
        .map(|(i, panel)| {
            let old = current.row_data(i).expect("counted above");
            let rows: Vec<_> = panel.rows.iter().collect();
            match updated(&old.rows, rows) {
                Some(rows) => SettingsPanel { rows, ..panel },
                None => SettingsPanel {
                    rows: old.rows,
                    ..panel
                },
            }
        })
        .collect();
    updated(current, panels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_change_in_place_while_the_length_stays() {
        let shown = ModelRc::new(VecModel::from(vec![1, 2, 3]));
        assert!(updated(&shown, vec![1, 5, 3]).is_none());
        assert_eq!(Some(5), shown.row_data(1));
        let longer = updated(&shown, vec![1, 2, 3, 4]).expect("a new model");
        assert_eq!(4, longer.row_count());
        assert!(updated(&ModelRc::default(), vec![1]).is_some());
    }
}
