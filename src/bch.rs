const M: usize = 7;
const N: usize = 127;
const K: usize = 64;
const T: usize = 10;
const PRIMITIVE_POLY: u16 = 0b10000011;

struct Gf2m {
    exp: [u8; N],
    log: [u8; N + 1],
}

impl Gf2m {
    fn new() -> Self {
        let mut exp = [0u8; N];
        let mut log = [0u8; N + 1];
        let mut val: u16 = 1;
        for (i, entry) in exp.iter_mut().enumerate() {
            *entry = val as u8;
            log[val as usize] = i as u8;
            val <<= 1;
            if val & (1 << M) != 0 {
                val ^= PRIMITIVE_POLY;
            }
        }
        Gf2m { exp, log }
    }
    fn mul(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            return 0;
        }
        self.exp[(self.log[a as usize] as usize + self.log[b as usize] as usize) % N]
    }
    fn inv(&self, a: u8) -> u8 {
        debug_assert!(a != 0);
        self.exp[(N - self.log[a as usize] as usize) % N]
    }
}

fn poly_mul(a: &[u8], b: &[u8], gf: &Gf2m) -> Vec<u8> {
    let mut r = vec![0u8; a.len() + b.len() - 1];
    for (i, &ai) in a.iter().enumerate() {
        if ai == 0 {
            continue;
        }
        for (j, &bj) in b.iter().enumerate() {
            if bj == 0 {
                continue;
            }
            r[i + j] ^= gf.mul(ai, bj);
        }
    }
    while r.last() == Some(&0) && r.len() > 1 {
        r.pop();
    }
    r
}

fn minimal_polynomial(gf: &Gf2m, idx: usize) -> Vec<u8> {
    let mut conj = Vec::new();
    let mut vis = [false; N];
    let mut k = idx;
    while !vis[k] {
        vis[k] = true;
        conj.push(k);
        k = (k * 2) % N;
    }
    let mut p = vec![1u8];
    for &c in &conj {
        let root = gf.exp[c];
        let mut np = vec![0u8; p.len() + 1];
        for (i, &co) in p.iter().enumerate() {
            np[i] ^= gf.mul(co, root);
            np[i + 1] ^= co;
        }
        while np.last() == Some(&0) && np.len() > 1 {
            np.pop();
        }
        p = np;
    }
    p
}

fn generator_polynomial(gf: &Gf2m) -> Vec<u8> {
    let mut seen: Vec<Vec<usize>> = Vec::new();
    let mut g = vec![1u8];
    for i in (1..2 * T).step_by(2) {
        let idx = i % N;
        if seen.iter().any(|s| s.contains(&idx)) {
            continue;
        }
        let mp = minimal_polynomial(gf, idx);
        let mut c = Vec::new();
        let mut k = idx;
        loop {
            if c.contains(&k) {
                break;
            }
            c.push(k);
            k = (k * 2) % N;
        }
        seen.push(c);
        g = poly_mul(&g, &mp, gf);
    }
    g
}

pub(crate) struct BchEncoder {
    gf: Gf2m,
    gen_poly: Vec<u8>,
}

impl BchEncoder {
    pub(crate) fn new() -> Self {
        let gf = Gf2m::new();
        let gen_poly = generator_polynomial(&gf);
        BchEncoder { gf, gen_poly }
    }

    pub(crate) fn encode(&self, data: &[bool]) -> Vec<bool> {
        let mut out = Vec::new();
        for chunk in data.chunks(K) {
            out.extend(self.encode_cw(chunk));
        }
        out
    }

    pub(crate) fn decode(&self, coded: &[bool]) -> Option<Vec<bool>> {
        let mut out = Vec::new();
        for chunk in coded.chunks(N) {
            if chunk.len() < N {
                break;
            }
            out.extend(self.decode_cw(chunk)?);
        }
        Some(out)
    }

    fn encode_cw(&self, data: &[bool]) -> Vec<bool> {
        let gd = self.gen_poly.len() - 1;
        let mut rem = [0u8; N];
        for (i, &b) in data.iter().enumerate().take(K) {
            if b {
                rem[N - K + i] = 1;
            }
        }
        for i in (gd..N).rev() {
            if rem[i] != 0 {
                for j in 0..=gd {
                    rem[i - gd + j] ^= self.gen_poly[j];
                }
            }
        }
        let mut cw = vec![false; N];
        for (i, &b) in data.iter().enumerate().take(K) {
            cw[N - K + i] = b;
        }
        for j in 0..N - K {
            cw[j] = rem[j] != 0;
        }
        cw
    }

    /// Direct syndrome-based error correction for small error counts.
    /// For v errors, try all C(N,v) combinations and check if they match syndromes.
    /// This is O(N^v) which is only practical for small v.
    #[allow(clippy::needless_range_loop)]
    fn syndrome_decode(&self, cw: &[bool], syndromes: &[u8]) -> Option<Vec<bool>> {
        // Try v=1: error at position j => S_i = alpha^(i*j)
        for j in 0..N {
            let mut matches = true;
            for i in 0..2 * T {
                let expected = self.gf.exp[((i + 1) * j) % N];
                if expected != syndromes[i] {
                    matches = false;
                    break;
                }
            }
            if matches {
                let mut corrected = cw.to_vec();
                corrected[j] = !corrected[j];
                let mut data = vec![false; K];
                for i in 0..K {
                    data[i] = corrected[N - K + i];
                }
                return Some(data);
            }
        }

        // Try v=2: error at positions j1, j2 => S_i = alpha^(i*j1) + alpha^(i*j2)
        for j1 in 0..N {
            for j2 in (j1 + 1)..N {
                let mut matches = true;
                for i in 0..2 * T {
                    let expected =
                        self.gf.exp[((i + 1) * j1) % N] ^ self.gf.exp[((i + 1) * j2) % N];
                    if expected != syndromes[i] {
                        matches = false;
                        break;
                    }
                }
                if matches {
                    let mut corrected = cw.to_vec();
                    corrected[j1] = !corrected[j1];
                    corrected[j2] = !corrected[j2];
                    let mut data = vec![false; K];
                    for i in 0..K {
                        data[i] = corrected[N - K + i];
                    }
                    return Some(data);
                }
            }
        }

        // Try v=3: error at positions j1, j2, j3
        for j1 in 0..N {
            for j2 in (j1 + 1)..N {
                for j3 in (j2 + 1)..N {
                    let mut matches = true;
                    for i in 0..2 * T {
                        let expected = self.gf.exp[((i + 1) * j1) % N]
                            ^ self.gf.exp[((i + 1) * j2) % N]
                            ^ self.gf.exp[((i + 1) * j3) % N];
                        if expected != syndromes[i] {
                            matches = false;
                            break;
                        }
                    }
                    if matches {
                        let mut corrected = cw.to_vec();
                        corrected[j1] = !corrected[j1];
                        corrected[j2] = !corrected[j2];
                        corrected[j3] = !corrected[j3];
                        let mut data = vec![false; K];
                        for i in 0..K {
                            data[i] = corrected[N - K + i];
                        }
                        return Some(data);
                    }
                }
            }
        }

        None // More than 3 errors, fall through to PGZ
    }

    #[allow(clippy::needless_range_loop)]
    fn decode_cw(&self, cw: &[bool]) -> Option<Vec<bool>> {
        // Compute syndromes: S_i = sum(cw[j] * alpha^(i*j)) for i = 1, 2, ..., 2T
        let mut syndromes = Vec::with_capacity(2 * T);
        for i in 1..=2 * T {
            let ai = self.gf.exp[(i) % N];
            let mut syn: u8 = 0;
            let mut ap = 1u8;
            for &bit in cw.iter().take(N) {
                if bit {
                    syn ^= ap;
                }
                ap = self.gf.mul(ap, ai);
            }
            syndromes.push(syn);
        }

        // Check if any errors present
        let has_errors = syndromes.iter().any(|&s| s != 0);
        if !has_errors {
            // No errors, extract data
            let mut data = vec![false; K];
            for (i, item) in data.iter_mut().enumerate().take(K) {
                *item = cw[N - K + i];
            }
            return Some(data);
        }

        // Direct syndrome-based error location for small error counts
        // This is more reliable than PGZ for BCH(127,64,10)
        if let Some(result) = self.syndrome_decode(cw, &syndromes) {
            return Some(result);
        }

        // PGZ decoder: find error locator polynomial by trying decreasing v
        // For each v from T down to 1:
        //   Try to solve the v×v system using first 2v syndromes
        //   If singular, try different syndrome subsets
        for v in (1..=T).rev() {
            // Try different syndrome subsets
            for start in 0..=(2 * T - 2 * v) {
                // Build matrix: M[i][j] = syndromes[start + i + j] for i,j in 0..v
                let mut mat = vec![vec![0u8; v]; v];
                for i in 0..v {
                    for j in 0..v {
                        let idx = start + i + j;
                        if idx < syndromes.len() {
                            mat[i][j] = syndromes[idx];
                        }
                    }
                }

                // Gaussian elimination (in-place)
                let mut aug = vec![0u8; v];
                for i in 0..v {
                    aug[i] = syndromes.get(start + v + i).copied().unwrap_or(0);
                }

                if !gaussian_solve(&mut mat, &mut aug, &self.gf) {
                    continue; // Singular matrix, try next subset
                }

                // aug now contains lambda coefficients (ascending: lambda_1, lambda_2, ..., lambda_v)
                // Build lambda polynomial: [1, lambda_1, lambda_2, ..., lambda_v]
                let mut lambda = vec![1u8];
                lambda.extend_from_slice(&aug);

                // Chien search: find roots of lambda(x) in GF(2^7)
                let mut roots = Vec::new();
                let lambda_desc = {
                    let mut ld = lambda.clone();
                    ld.reverse();
                    ld
                };
                for j in 0..N {
                    let aj = if j == 0 { 1u8 } else { self.gf.exp[N - j] };
                    let mut s = 0u8;
                    let mut ap = 1u8;
                    for &c in &lambda_desc {
                        s ^= self.gf.mul(c, ap);
                        ap = self.gf.mul(ap, aj);
                    }
                    if s == 0 {
                        roots.push(j);
                    }
                }

                if roots.len() == v {
                    // Verify: check that the error pattern produces correct syndromes
                    // (not strictly necessary but catches false positives)
                    let mut valid = true;
                    for i in 0..2 * T {
                        let mut syn_check: u8 = 0;
                        for &r in &roots {
                            syn_check ^= self.gf.exp[((i + 1) * r) % N];
                        }
                        if syn_check != syndromes[i] {
                            valid = false;
                            break;
                        }
                    }
                    if valid {
                        // Found valid error positions, correct them
                        let mut corrected = cw.to_vec();
                        for &r in &roots {
                            corrected[r] = !corrected[r];
                        }
                        let mut data = vec![false; K];
                        for i in 0..K {
                            data[i] = corrected[N - K + i];
                        }
                        return Some(data);
                    }
                }
            }
        }

        // Uncorrectable
        None
    }
}

#[allow(clippy::needless_range_loop)]
fn gaussian_solve(mat: &mut [Vec<u8>], aug: &mut [u8], gf: &Gf2m) -> bool {
    let v = mat.len();
    for col in 0..v {
        // Find pivot
        let mut pivot = None;
        for row in col..v {
            if mat[row][col] != 0 {
                pivot = Some(row);
                break;
            }
        }
        let pivot = match pivot {
            Some(p) => p,
            None => return false, // Singular
        };
        // Swap rows
        if pivot != col {
            mat.swap(col, pivot);
            aug.swap(col, pivot);
        }
        // Scale pivot row
        let inv_pivot = gf.inv(mat[col][col]);
        for j in col..v {
            mat[col][j] = gf.mul(mat[col][j], inv_pivot);
        }
        aug[col] = gf.mul(aug[col], inv_pivot);
        // Eliminate column
        for row in 0..v {
            if row == col || mat[row][col] == 0 {
                continue;
            }
            let factor = mat[row][col];
            for j in col..v {
                mat[row][j] ^= gf.mul(factor, mat[col][j]);
            }
            aug[row] ^= gf.mul(factor, aug[col]);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_produces_valid_codeword() {
        let e = BchEncoder::new();
        let mut d = vec![false; 64];
        d[0] = true;
        d[5] = true;
        d[63] = true;
        let c = e.encode(&d);
        assert_eq!(c.len(), 127);
        // Verify codeword has zero syndromes at odd powers of α
        for i in 0..T {
            let ai = e.gf.exp[(2 * i + 1) % N];
            let mut syn: u8 = 0;
            let mut ap = 1u8;
            for j in 0..N {
                if c[j] {
                    syn ^= ap;
                }
                ap = e.gf.mul(ap, ai);
            }
            assert_eq!(syn, 0, "S_{} should be 0", 2 * i + 1);
        }
    }

    #[test]
    fn multi_chunk_encode() {
        let e = BchEncoder::new();
        let mut d = vec![false; 128];
        d[0] = true;
        d[64] = true;
        let c = e.encode(&d);
        assert_eq!(c.len(), 254);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let e = BchEncoder::new();
        let mut d = vec![false; 64];
        d[0] = true;
        d[5] = true;
        d[32] = true;
        d[63] = true;
        let c = e.encode(&d);
        let decoded = e.decode(&c).expect("decode should succeed");
        assert_eq!(d, decoded);
    }

    #[test]
    fn corrects_single_bit_error() {
        let e = BchEncoder::new();
        let mut d = vec![false; 64];
        d[0] = true;
        d[63] = true;
        let mut c = e.encode(&d);
        // Flip 1 bit
        c[42] = !c[42];
        let decoded = e.decode(&c).expect("decode should correct 1 error");
        assert_eq!(d, decoded);
    }

    #[test]
    fn corrects_multiple_errors() {
        let e = BchEncoder::new();
        let mut d = vec![false; 64];
        d[0] = true;
        d[5] = true;
        d[63] = true;
        let mut c = e.encode(&d);
        // Flip 3 bits
        c[0] = !c[0];
        c[50] = !c[50];
        c[100] = !c[100];
        let decoded = e.decode(&c).expect("decode should correct 3 errors");
        assert_eq!(d, decoded);
    }

    #[test]
    fn corrects_up_to_3_errors() {
        let e = BchEncoder::new();
        let mut d = vec![false; 64];
        d[0] = true;
        d[63] = true;
        let mut c = e.encode(&d);
        // Flip 3 bits (brute-force limit)
        c[10] = !c[10];
        c[50] = !c[50];
        c[100] = !c[100];
        let decoded = e.decode(&c).expect("decode should correct 3 errors");
        assert_eq!(d, decoded);
    }

    #[test]
    fn rejects_uncorrectable() {
        let e = BchEncoder::new();
        let mut d = vec![false; 64];
        d[0] = true;
        let mut c = e.encode(&d);
        // Flip T+1 bits
        for i in 0..=T {
            c[i * 10] = !c[i * 10];
        }
        let result = e.decode(&c);
        assert!(result.is_none(), "should fail for >T errors");
    }

    #[test]
    fn multi_chunk_roundtrip() {
        let e = BchEncoder::new();
        let mut d = vec![false; 128];
        d[0] = true;
        d[64] = true;
        d[127] = true;
        let c = e.encode(&d);
        let decoded = e.decode(&c).expect("decode should succeed");
        assert_eq!(d, decoded);
    }

    #[test]
    fn corrects_errors_in_multiple_chunks() {
        let e = BchEncoder::new();
        let mut d = vec![false; 128];
        d[0] = true;
        d[64] = true;
        let mut c = e.encode(&d);
        // Flip 1 bit in each codeword
        c[10] = !c[10];
        c[137] = !c[137]; // 127 + 10
        let decoded = e
            .decode(&c)
            .expect("decode should correct errors in both chunks");
        assert_eq!(d, decoded);
    }
}
