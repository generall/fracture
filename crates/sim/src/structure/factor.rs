//! Sparse positive-definite LDLᵀ with deterministic cell-block minimum-degree
//! ordering. Elimination cliques supply the fill pattern; row-wise triangular
//! solves construct the numeric columns. No stiffness shifts or dropped fill.
use std::collections::BTreeSet;

pub(super) struct Factor {
    order: Vec<usize>,
    columns: Vec<Vec<(usize, f64)>>,
    diagonal: Vec<f64>,
}

impl Factor {
    pub fn new(matrix: &[Vec<(usize, f64)>]) -> Option<Self> {
        let size = matrix.len();
        let cells = size / 3;
        let mut graph = vec![BTreeSet::new(); cells];
        for (row, entries) in matrix.iter().enumerate() {
            for &(column, value) in entries {
                if row / 3 != column / 3 && value != 0.0 {
                    graph[row / 3].insert(column / 3);
                }
            }
        }
        let mut cell_order = Vec::with_capacity(cells);
        let mut eliminated = vec![false; cells];
        let mut later = Vec::with_capacity(cells);
        let mut fill = 0;
        for _ in 0..cells {
            let cell = (0..cells)
                .filter(|&c| !eliminated[c])
                .min_by_key(|&c| (graph[c].len(), c))?;
            let neighbors: Vec<_> = graph[cell].iter().copied().collect();
            fill += neighbors.len() * 9 + 3;
            // Bound scratch/factor memory for pathological authored graphs.
            if fill > 2_000_000 {
                return None;
            }
            eliminated[cell] = true;
            for &a in &neighbors {
                graph[a].remove(&cell);
                for &b in &neighbors {
                    if a != b {
                        graph[a].insert(b);
                    }
                }
            }
            graph[cell].clear();
            cell_order.push(cell);
            later.push(neighbors);
        }
        let order: Vec<_> = cell_order
            .iter()
            .flat_map(|&c| [3 * c, 3 * c + 1, 3 * c + 2])
            .collect();
        let mut inverse = vec![0; size];
        for (new, &old) in order.iter().enumerate() {
            inverse[old] = new;
        }
        // A cell's three variables are eliminated together. Dense 3x3 blocks
        // give a conservative symbolic pattern, even for axis-aligned faces.
        let mut predecessors = vec![Vec::new(); size];
        let mut capacity = vec![0; size];
        for (cell, neighbors) in later.iter().enumerate() {
            for d in 0..3 {
                let column = 3 * cell + d;
                for entries in &mut predecessors[column + 1..3 * cell + 3] {
                    entries.push(column);
                    capacity[column] += 1;
                }
                for &other in neighbors {
                    let start = inverse[3 * other];
                    for entries in &mut predecessors[start..start + 3] {
                        entries.push(column);
                        capacity[column] += 1;
                    }
                }
            }
        }
        let mut columns: Vec<Vec<(usize, f64)>> =
            capacity.into_iter().map(Vec::with_capacity).collect();
        let mut diagonal = vec![0.0; size];
        let mut work = vec![0.0; size];
        for row in 0..size {
            for &(old_column, value) in &matrix[order[row]] {
                let column = inverse[old_column];
                if column <= row {
                    work[column] += value;
                }
            }
            let mut pivot = work[row];
            work[row] = 0.0;
            for &column in &predecessors[row] {
                let numerator = work[column];
                work[column] = 0.0;
                for &(other, value) in &columns[column] {
                    work[other] -= value * numerator;
                }
                let entry = numerator / diagonal[column];
                pivot -= entry * numerator;
                columns[column].push((row, entry));
            }
            if !pivot.is_finite() || pivot <= 1e-14 {
                return None;
            }
            diagonal[row] = pivot;
        }
        Some(Self {
            order,
            columns,
            diagonal,
        })
    }

    pub fn solve(&self, rhs: &[f64], out: &mut [f64]) {
        let mut x: Vec<_> = self.order.iter().map(|&i| rhs[i]).collect();
        for (column, entries) in self.columns.iter().enumerate() {
            for &(row, value) in entries {
                x[row] -= value * x[column];
            }
        }
        for (x, diagonal) in x.iter_mut().zip(&self.diagonal) {
            *x /= diagonal;
        }
        for column in (0..x.len()).rev() {
            for &(row, value) in &self.columns[column] {
                x[column] -= value * x[row];
            }
        }
        for (new, &old) in self.order.iter().enumerate() {
            out[old] = x[new];
        }
    }
}
