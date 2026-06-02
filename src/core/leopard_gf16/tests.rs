use super::*;
use super::tables::build_tables16;
use super::ops::{gf16_mul, fft_dit2_16, ifft_dit2_16, slice_xor_u16, mulgf16, mul_log16, add_mod16, sub_mod16};

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
fn test_leopard_gf16_check_luts() {
    let tables = build_tables16();
    // Check the LFSR cycle directly: x = 2*x mod polynomial
    let mut x: u32 = 1;
    let mut count = 0usize;
    loop {
        count += 1;
        x <<= 1;
        if x >= ORDER16 as u32 {
            x ^= super::POLYNOMIAL16;
        }
        if x == 1 { break; }
        if count > MODULUS16 { break; }
    }
    eprintln!("LFSR cycle length: {count} (expected {MODULUS16})");
    eprintln!("POLYNOMIAL16 = {:#06x}", super::POLYNOMIAL16);
    eprintln!("exp_lut[0]={}, exp_lut[1]={}, exp_lut[2]={}", tables.exp_lut[0], tables.exp_lut[1], tables.exp_lut[2]);
    eprintln!("log_lut[0]={}, log_lut[1]={}, log_lut[2]={}", tables.log_lut[0], tables.log_lut[1], tables.log_lut[2]);
    eprintln!("log_walsh[0]={}, log_walsh[1]={}, log_walsh[2]={}", tables.log_walsh[0], tables.log_walsh[1], tables.log_walsh[2]);
    eprintln!("fft_skew[0]={}, fft_skew[1]={}, fft_skew[2]={}", tables.fft_skew[0], tables.fft_skew[1], tables.fft_skew[2]);
    assert_eq!(count, MODULUS16, "LFSR should cycle through all 65535 nonzero elements");
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
    slice_xor_u16(&mut b, &a);
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
                .map(|j| ((i * 137 + j * 31) & 0xFF) as u8)
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

    assert_eq!(parity, parity2, "re-encoding should produce same parity");
}

/// Test FWHT self-inverse property: fwht(fwht(x)) = x.
#[test]
fn test_leopard_gf16_fwht_self_inverse() {
    // Build a small test vector.
    let mut data = [0u16; super::ORDER16];
    data[0] = 42;
    data[1] = 1000;
    data[2] = 65534;
    data[3] = 12345;
    data[4] = 7;

    let original = data;

    // Apply FWHT twice — should recover original.
    super::ops::fwht16_mtrunc(&mut data, super::ORDER16);
    super::ops::fwht16_variable(&mut data[..super::ORDER16]);

    for i in 0..16 {
        assert_eq!(
            data[i], original[i],
            "position {i}: got {:#06x}, expected {:#06x}",
            data[i], original[i]
        );
    }
    eprintln!("FWHT self-inverse: OK (first 16 positions match)");
}

/// Test encode-decode with non-uniform per-position data.
#[test]
fn test_leopard_gf16_reconstruct_nonuniform() {
    let data_shards = 3;
    let parity_shards = 2;
    let total_shards = data_shards + parity_shards;
    let shard_size = 64;

    // Non-uniform data: each byte position has different values across shards.
    let data: Vec<Vec<u8>> = (0..data_shards)
        .map(|i| {
            (0..shard_size)
                .map(|j| ((i * shard_size + j) & 0xFF) as u8)
                .collect()
        })
        .collect();
    let mut parity: Vec<Vec<u8>> = vec![vec![0u8; shard_size]; parity_shards];

    encode::encode_with_tables16(data_shards, parity_shards, &data, &mut parity).unwrap();

    let mut shards: Vec<Vec<u8>> = Vec::new();
    for d in &data {
        shards.push(d.clone());
    }
    for p in &parity {
        shards.push(p.clone());
    }

    let original_0 = shards[0].clone();
    shards[0] = vec![0u8; shard_size];

    let present: Vec<bool> = (0..total_shards).map(|i| i != 0).collect();
    let input_snapshots: Vec<Option<Vec<u8>>> = shards
        .iter()
        .enumerate()
        .map(|(i, s)| if i != 0 { Some(s.clone()) } else { None })
        .collect();
    let input_data: Vec<Option<&[u8]>> = input_snapshots.iter().map(|o| o.as_deref()).collect();
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

    assert_eq!(outputs[0], original_0.as_slice(), "recovered shard 0 should match original");
}

/// Verify basic GF16 arithmetic properties.
#[test]
fn test_leopard_gf16_arithmetic() {
    let tables = build_tables16();

    // Verify log/exp consistency: exp_lut[log_lut[x]] = x for all nonzero x.
    for x in 1u16..65535 {
        let log_x = tables.log_lut[x as usize];
        let recovered = tables.exp_lut[log_x as usize];
        assert_eq!(recovered, x, "exp(log({x})) != {x}, got {recovered}");
    }

    // Verify exp_lut[0] = 1 (multiplicative identity).
    assert_eq!(tables.exp_lut[0], 1);

    // Verify log_lut[1] = 0 (log of identity).
    assert_eq!(tables.log_lut[1], 0);

    // Verify gf16_mul(a, 1) = a for all a.
    for a in 0u16..65535 {
        let result = gf16_mul(a, 1, &tables.log_lut, &tables.exp_lut);
        assert_eq!(result, a, "gf16_mul({a}, 1) = {result}");
    }

    // Verify gf16_mul(0, x) = 0 for all x.
    for x in 0u16..65535 {
        assert_eq!(gf16_mul(0, x, &tables.log_lut, &tables.exp_lut), 0);
        assert_eq!(gf16_mul(x, 0, &tables.log_lut, &tables.exp_lut), 0);
    }

    // Verify mul_log16(a, log_lut[b]) = gf16_mul(a, b).
    let test_vals = [1u16, 2, 3, 100, 1000, 32768, 65534];
    for &a in &test_vals {
        for &b in &test_vals {
            let r1 = gf16_mul(a, b, &tables.log_lut, &tables.exp_lut);
            let r2 = mul_log16(a, tables.log_lut[b as usize], &tables.log_lut, &tables.exp_lut);
            assert_eq!(r1, r2, "gf16_mul({a},{b})={r1} != mul_log16({a},log({b}))={r2}");
        }
    }

    // Verify add_mod16/sub_mod16 roundtrip.
    for a in 0u16..100 {
        for b in 0u16..100 {
            let sum = add_mod16(a, b);
            let diff = sub_mod16(sum, b);
            assert_eq!(diff, a, "add_mod16/sub_mod16 roundtrip failed for ({a}, {b})");
        }
    }
}

/// Debug: print FFT skew values and plan structure for the 3+2 decode case.
#[test]
fn test_leopard_gf16_debug_skew_and_plans() {
    let tables = init_leopard_gf16_tables();

    // Print first few fft_skew values
    eprintln!("fft_skew[0..8] = {:?}", &tables.fft_skew[..8]);

    // Print log_walsh values for key positions
    eprintln!("log_walsh[0..8] = {:?}", &tables.log_walsh[..8]);

    // Build plans for 3+2 decode
    let data_shards = 3usize;
    let parity_shards = 2usize;
    let m = parity_shards.max(1).next_power_of_two(); // 2
    let work_size = m + data_shards; // 5
    let n = work_size.next_power_of_two(); // 8
    let input_count = 5usize; // m + last_present_data_index + 1

    eprintln!("m={m}, n={n}, work_size={work_size}, input_count={input_count}");

    // IFFT plan
    let ifft_plan = super::build_ifft_decode_dit16_plan(input_count, n, &*tables.fft_skew);
    eprintln!("IFFT plan: initial_blocks={:?}, later_blocks={:?}, final_stage={:?}, clear_start={}",
        ifft_plan.initial_blocks, ifft_plan.later_blocks, ifft_plan.final_stage, ifft_plan.clear_start);

    // FFT plan
    let fft_plan = super::build_fft_dit16_plan(work_size, n, &*tables.fft_skew);
    eprintln!("FFT plan: stage4_blocks={:?}, final_stage={:?}",
        fft_plan.stage4_blocks, fft_plan.final_stage);
}

/// Test encode+decode roundtrip directly using encode_with_tables16 and reconstruct_with_tables16.
#[test]
fn test_leopard_gf16_encode_decode_roundtrip() {
    let data_shards = 3;
    let parity_shards = 2;
    let total_shards = data_shards + parity_shards;
    let shard_size = 64;

    let data: Vec<Vec<u8>> = (0..data_shards)
        .map(|i| {
            (0..shard_size)
                .map(|j| ((i * shard_size + j) & 0xFF) as u8)
                .collect()
        })
        .collect();
    let mut parity: Vec<Vec<u8>> = vec![vec![0u8; shard_size]; parity_shards];

    encode::encode_with_tables16(data_shards, parity_shards, &data, &mut parity).unwrap();

    // Debug: print first few u16 values of each shard
    for i in 0..data_shards {
        let u16_vals: Vec<u16> = (0..4).map(|j| {
            let lo = data[i][j*2] as u16;
            let hi = data[i][j*2+1] as u16;
            lo | (hi << 8)
        }).collect();
        eprintln!("  data[{i}] u16[0..4] = {:04x?}", u16_vals);
    }
    for i in 0..parity_shards {
        let u16_vals: Vec<u16> = (0..4).map(|j| {
            let lo = parity[i][j*2] as u16;
            let hi = parity[i][j*2+1] as u16;
            lo | (hi << 8)
        }).collect();
        eprintln!("  parity[{i}] u16[0..4] = {:04x?}", u16_vals);
    }

    // Lose shard 0 (data shard).
    let original_0 = data[0].clone();
    let mut shards: Vec<Vec<u8>> = Vec::new();
    for i in 0..data_shards {
        if i == 0 {
            shards.push(vec![0u8; shard_size]);
        } else {
            shards.push(data[i].clone());
        }
    }
    for p in &parity {
        shards.push(p.clone());
    }

    let present: Vec<bool> = (0..total_shards).map(|i| i != 0).collect();
    let input_snapshots: Vec<Option<Vec<u8>>> = shards
        .iter()
        .enumerate()
        .map(|(i, s)| if i != 0 { Some(s.clone()) } else { None })
        .collect();
    let input_data: Vec<Option<&[u8]>> = input_snapshots.iter().map(|o| o.as_deref()).collect();
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

    assert_eq!(outputs[0], original_0.as_slice(), "recovered shard 0 should match original");
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

/// Verify the full decode pipeline with manual step-by-step computation.
#[test]
fn test_leopard_gf16_decode_pipeline_manual() {
    let data_shards = 3;
    let parity_shards = 2;
    let total_shards = data_shards + parity_shards;
    let shard_size = 64;

    // Use uniform data for easier debugging.
    let data: Vec<Vec<u8>> = (0..data_shards)
        .map(|i| vec![(i + 1) as u8; shard_size])
        .collect();
    let mut parity: Vec<Vec<u8>> = vec![vec![0u8; shard_size]; parity_shards];

    encode::encode_with_tables16(data_shards, parity_shards, &data, &mut parity).unwrap();

    // Print parity values.
    for i in 0..parity_shards {
        let u16_vals: Vec<u16> = (0..4.min(shard_size/2)).map(|j| {
            let lo = parity[i][j*2] as u16;
            let hi = parity[i][j*2+1] as u16;
            lo | (hi << 8)
        }).collect();
        eprintln!("  parity[{i}] u16[0..4] = {:04x?}", u16_vals);
    }

    // Print data values (u16).
    for i in 0..data_shards {
        let u16_vals: Vec<u16> = (0..4.min(shard_size/2)).map(|j| {
            let lo = data[i][j*2] as u16;
            let hi = data[i][j*2+1] as u16;
            lo | (hi << 8)
        }).collect();
        eprintln!("  data[{i}] u16[0..4] = {:04x?}", u16_vals);
    }

    // Verify: encode then decode roundtrip for shard 0.
    let original_0 = data[0].clone();
    let mut shards: Vec<Vec<u8>> = Vec::new();
    for i in 0..data_shards {
        if i == 0 {
            shards.push(vec![0u8; shard_size]);
        } else {
            shards.push(data[i].clone());
        }
    }
    for p in &parity {
        shards.push(p.clone());
    }

    let present: Vec<bool> = (0..total_shards).map(|i| i != 0).collect();
    let input_snapshots: Vec<Option<Vec<u8>>> = shards
        .iter()
        .enumerate()
        .map(|(i, s)| if i != 0 { Some(s.clone()) } else { None })
        .collect();
    let input_data: Vec<Option<&[u8]>> = input_snapshots.iter().map(|o| o.as_deref()).collect();
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

    assert_eq!(outputs[0], original_0.as_slice(), "recovered shard 0 should match original");
}

#[test]
fn test_leopard_gf16_reconstruct_simple_1plus1() {
    let data_shards = 1;
    let parity_shards = 1;
    let shard_size = 64;

    let data: Vec<Vec<u8>> = vec![(0..shard_size).map(|j| ((j + 1) & 0xFF) as u8).collect()];
    let mut parity: Vec<Vec<u8>> = vec![vec![0u8; shard_size]; parity_shards];

    encode::encode_with_tables16(data_shards, parity_shards, &data, &mut parity).unwrap();

    let mut shards: Vec<Vec<u8>> = Vec::new();
    shards.push(vec![0u8; shard_size]); // missing data shard 0
    shards.push(parity[0].clone());

    let original_0 = data[0].clone();

    let present: Vec<bool> = vec![false, true];
    let input_snapshots: Vec<Option<Vec<u8>>> = shards
        .iter()
        .enumerate()
        .map(|(i, s)| if i != 0 { Some(s.clone()) } else { None })
        .collect();
    let input_data: Vec<Option<&[u8]>> = input_snapshots.iter().map(|o| o.as_deref()).collect();
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

    assert_eq!(outputs[0], original_0.as_slice(), "recovered shard 0 should match original");
}
