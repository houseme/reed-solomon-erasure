use super::*;
use super::tables::build_tables16;
use super::ops::{gf16_mul, fft_dit2_16, ifft_dit2_16, slice_xor_u16, mulgf16};

#[test]
fn test_leopard_gf16_tables_shapes() {
    let tables = build_tables16();
    assert_eq!(tables.log_lut.len(), ORDER16);
    assert_eq!(tables.exp_lut.len(), ORDER16 * 2);
    assert_eq!(tables.fft_skew.len(), MODULUS16);
    assert_eq!(tables.log_walsh.len(), ORDER16);
}

#[test]
fn test_leopard_gf16_log_exp_roundtrip() {
    let tables = build_tables16();
    for x in 1..ORDER16 {
        let log_x = tables.log_lut[x];
        let recovered = tables.exp_lut[log_x as usize];
        assert_eq!(recovered, x as u16, "x={x}");
    }
}

#[test]
fn test_leopard_gf16_mul_identity() {
    let tables = build_tables16();
    for x in 0..ORDER16 {
        let result = gf16_mul(x as u16, 1, &tables.log_lut, &tables.exp_lut);
        assert_eq!(result, x as u16, "x={x}");
    }
}

#[test]
fn test_leopard_gf16_mul_zero() {
    let tables = build_tables16();
    for x in 0..ORDER16 {
        assert_eq!(gf16_mul(0, x as u16, &tables.log_lut, &tables.exp_lut), 0);
        assert_eq!(gf16_mul(x as u16, 0, &tables.log_lut, &tables.exp_lut), 0);
    }
}

#[test]
fn test_leopard_gf16_fft_ifft_roundtrip() {
    let tables = build_tables16();
    let n = 16;
    let original: Vec<u16> = (0..n).map(|i| (i as u16 * 137 + 42) % 65535).collect();
    let mut data = original.clone();

    // Apply FFT butterflies using split_at_mut.
    for i in 0..n / 2 {
        let (left, right) = data.split_at_mut(n / 2);
        fft_dit2_16(&mut left[i..i + 1], &mut right[i..i + 1], (i + 1) as u16, &tables);
    }

    // Apply inverse FFT butterflies (reverse order).
    for i in (0..n / 2).rev() {
        let (left, right) = data.split_at_mut(n / 2);
        ifft_dit2_16(&mut left[i..i + 1], &mut right[i..i + 1], (i + 1) as u16, &tables);
    }

    assert_eq!(data, original, "FFT -> IFFT should recover original data");
}

#[test]
fn test_leopard_gf16_mulgf16_basic() {
    let tables = build_tables16();
    let input = vec![1u16, 2, 3, 4, 5];
    let mut output = vec![0u16; 5];

    // Multiply by g^0 = 1 (identity).
    mulgf16(&mut output, &input, 0, &tables);
    assert_eq!(output, input, "mul by g^0 should be identity");

    // Multiply by g^MODULUS16 = g^65535 = 1 (wraparound).
    output.fill(0);
    mulgf16(&mut output, &input, super::MODULUS16 as u16, &tables);
    assert_eq!(output, input, "mul by g^65535 should be identity");
}

#[test]
fn test_leopard_gf16_slice_xor_u16() {
    let a = vec![0x1234u16, 0x5678, 0x9ABC, 0xDEF0];
    let mut b = vec![0xFFFFu16, 0x0000, 0x1234, 0x5678];
    slice_xor_u16(&a, &mut b);
    assert_eq!(b, vec![0x1234 ^ 0xFFFF, 0x5678 ^ 0x0000, 0x9ABC ^ 0x1234, 0xDEF0 ^ 0x5678]);
}

#[test]
fn test_leopard_gf16_encode_basic() {
    let data_shards = 4;
    let parity_shards = 2;
    let shard_size = 128;

    let data: Vec<Vec<u8>> = (0..data_shards)
        .map(|i| {
            (0..shard_size)
                .map(|j| ((i * shard_size + j) & 0xFF) as u8)
                .collect()
        })
        .collect();
    let mut parity: Vec<Vec<u8>> = vec![vec![0u8; shard_size]; parity_shards];

    let driver =
        encode::encode_with_tables16(data_shards, parity_shards, &data, &mut parity).unwrap();

    assert_eq!(driver.shard_size, shard_size);
    assert_eq!(driver.m, 2);

    let all_zeros = parity.iter().all(|p| p.iter().all(|&b| b == 0));
    assert!(!all_zeros, "parity should be non-zero after encoding");
}

#[test]
fn test_leopard_gf16_encode_verify_roundtrip() {
    let data_shards = 3;
    let parity_shards = 2;
    let shard_size = 64;

    let data: Vec<Vec<u8>> = (0..data_shards)
        .map(|i| vec![i as u8 + 1; shard_size])
        .collect();
    let mut parity: Vec<Vec<u8>> = vec![vec![0u8; shard_size]; parity_shards];

    encode::encode_with_tables16(data_shards, parity_shards, &data, &mut parity).unwrap();

    let mut parity2: Vec<Vec<u8>> = vec![vec![0u8; shard_size]; parity_shards];
    encode::encode_with_tables16(data_shards, parity_shards, &data, &mut parity2).unwrap();

    // Debug: print both parity results
    eprintln!("parity[0][..8] = {:?}", &parity[0][..8]);
    eprintln!("parity[1][..8] = {:?}", &parity[1][..8]);
    eprintln!("parity2[0][..8] = {:?}", &parity2[0][..8]);
    eprintln!("parity2[1][..8] = {:?}", &parity2[1][..8]);

    assert_eq!(parity, parity2, "re-encoding should produce same parity");
}

#[test]
fn test_leopard_gf16_reconstruct_single_missing() {
    let data_shards = 3;
    let parity_shards = 2;
    let total_shards = data_shards + parity_shards;
    let shard_size = 64;

    let mut shards: Vec<Vec<u8>> = (0..total_shards)
        .map(|i| vec![(i + 1) as u8; shard_size])
        .collect();

    // Encode parity.
    {
        let (data_part, parity_part) = shards.split_at_mut(data_shards);
        let data_refs: Vec<&[u8]> = data_part.iter().map(|d| d.as_slice()).collect();
        let mut parity_refs: Vec<&mut [u8]> = parity_part.iter_mut().map(|p| p.as_mut_slice()).collect();
        encode::encode_with_tables16(data_shards, parity_shards, &data_refs, &mut parity_refs).unwrap();
    }

    let original_0 = shards[0].clone();

    // Debug: print parity values
    eprintln!("shard[0][..8] = {:?}", &shards[0][..8]);
    eprintln!("shard[1][..8] = {:?}", &shards[1][..8]);
    eprintln!("shard[2][..8] = {:?}", &shards[2][..8]);
    eprintln!("shard[3][..8] = {:?}", &shards[3][..8]);
    eprintln!("shard[4][..8] = {:?}", &shards[4][..8]);

    shards[0] = vec![0u8; shard_size];

    let present: Vec<bool> = (0..total_shards).map(|i| i != 0).collect();
    let input_snapshots: Vec<Option<Vec<u8>>> = shards
        .iter()
        .enumerate()
        .map(|(i, s)| if i != 0 { Some(s.clone()) } else { None })
        .collect();
    let input_data: Vec<Option<&[u8]>> = input_snapshots
        .iter()
        .map(|o| o.as_deref())
        .collect();
    let mut outputs: Vec<&mut [u8]> = shards.iter_mut().map(|s| s.as_mut_slice()).collect();

    decode::reconstruct_with_tables16(
        &present,
        &mut outputs,
        &input_data,
        data_shards,
        parity_shards,
        init_leopard_gf16_tables(),
    )
    .unwrap();

    eprintln!("recovered[0][..8] = {:?}", &outputs[0][..8]);
    assert_eq!(outputs[0], original_0.as_slice(), "recovered shard 0 should match original");
}

#[test]
fn test_leopard_gf16_reconstruct_multiple_missing() {
    let data_shards = 4;
    let parity_shards = 3;
    let total_shards = data_shards + parity_shards;
    let shard_size = 128;

    let mut shards: Vec<Vec<u8>> = (0..total_shards)
        .map(|i| {
            (0..shard_size)
                .map(|j| ((i * 137 + j * 31) & 0xFF) as u8)
                .collect()
        })
        .collect();

    {
        let (data_part, parity_part) = shards.split_at_mut(data_shards);
        let data_refs: Vec<&[u8]> = data_part.iter().map(|d| d.as_slice()).collect();
        let mut parity_refs: Vec<&mut [u8]> = parity_part.iter_mut().map(|p| p.as_mut_slice()).collect();
        encode::encode_with_tables16(data_shards, parity_shards, &data_refs, &mut parity_refs).unwrap();
    }

    let erased = [0, 2, 5];
    let originals: Vec<(usize, Vec<u8>)> = erased.iter().map(|&i| (i, shards[i].clone())).collect();
    for &i in &erased {
        shards[i] = vec![0u8; shard_size];
    }

    let present: Vec<bool> = (0..total_shards).map(|i| !erased.contains(&i)).collect();
    let input_snapshots: Vec<Option<Vec<u8>>> = shards
        .iter()
        .enumerate()
        .map(|(i, s)| if !erased.contains(&i) { Some(s.clone()) } else { None })
        .collect();
    let input_data: Vec<Option<&[u8]>> = input_snapshots
        .iter()
        .map(|o| o.as_deref())
        .collect();
    let mut outputs: Vec<&mut [u8]> = shards.iter_mut().map(|s| s.as_mut_slice()).collect();

    decode::reconstruct_with_tables16(
        &present,
        &mut outputs,
        &input_data,
        data_shards,
        parity_shards,
        init_leopard_gf16_tables(),
    )
    .unwrap();

    for (i, original) in &originals {
        assert_eq!(outputs[*i], original.as_slice(), "recovered shard {i} should match original");
    }
}
