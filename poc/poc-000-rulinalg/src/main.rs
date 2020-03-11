// https://github.com/AtheMathmo/rulinalg/issues/201
use rulinalg::matrix;
use rulinalg::matrix::BaseMatrixMut;

fn main() {
    let mut mat = matrix![0];

    let mut row = mat.row_mut(0);

    // this creates mutable aliases to the same location
    let raw_slice1 = row.raw_slice_mut();
    let raw_slice2 = row.raw_slice_mut();

    assert_eq!(raw_slice1[0], 0);
    raw_slice2[0] = 1;
    assert_eq!(raw_slice1[0], 0);
}
