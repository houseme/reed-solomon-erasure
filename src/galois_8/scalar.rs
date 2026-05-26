pub(crate) fn mul_slice_pure_rust(c: u8, input: &[u8], out: &mut [u8]) {
    let mt = &super::MUL_TABLE[c as usize];
    assert_eq!(input.len(), out.len());
    for (i, o) in input.iter().zip(out.iter_mut()) {
        *o = mt[*i as usize];
    }
}

pub(crate) fn mul_slice_xor_pure_rust(c: u8, input: &[u8], out: &mut [u8]) {
    let mt = &super::MUL_TABLE[c as usize];
    assert_eq!(input.len(), out.len());
    for (i, o) in input.iter().zip(out.iter_mut()) {
        *o ^= mt[*i as usize];
    }
}

#[cfg(test)]
pub(crate) fn slice_xor(input: &[u8], out: &mut [u8]) {
    assert_eq!(input.len(), out.len());
    for (i, o) in input.iter().zip(out.iter_mut()) {
        *o ^= *i;
    }
}
