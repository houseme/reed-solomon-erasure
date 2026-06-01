use super::*;

#[test]
fn test_leopard_gf8_tables_initialize_expected_shapes() {
    let tables = init_leopard_gf8_tables();
    assert_eq!(MODULUS8, tables.fft_skew.len());
    assert_eq!(ORDER8, tables.log_walsh.len());
    assert_eq!(ORDER8, tables.log_lut.len());
    assert_eq!(ORDER8, tables.exp_lut.len());
    assert_eq!(ORDER8, tables.mul_luts.len());
    assert_eq!(255, tables.log_lut[0]);
    assert_eq!(1, tables.exp_lut[0]);
}

#[test]
fn test_leopard_gf8_encode_driver_expected_parameters() {
    let driver = build_leopard_gf8_encode_driver(64, 32, 1024 * 1024).unwrap();
    assert_eq!(32, driver.m);
    assert_eq!(32, driver.mtrunc);
    assert_eq!(0, driver.last_count);
    assert_eq!(WORK_SIZE8, driver.chunk_size);
    assert_eq!(64, driver.work_slices);
    assert_eq!(31, driver.skew_offset);
}

#[test]
fn test_print_tables() {
    let tables = init_leopard_gf8_tables();
    println!("fft_skew = {:?}", &tables.fft_skew[..]);
    println!("log_walsh = {:?}", &tables.log_walsh[..16]);
    println!("log_lut[0..16] = {:?}", &tables.log_lut[..16]);
    println!("exp_lut[0..16] = {:?}", &tables.exp_lut[..16]);
}

/// Direct encode-then-decode roundtrip at the low-level API.
#[test]
fn test_encode_decode_roundtrip_direct() {
    let data_shards = 2usize;
    let parity_shards = 2usize;
    let shard_size = 64usize;
    let total = data_shards + parity_shards;
    let tables = init_leopard_gf8_tables();

    // Create deterministic data.
    let mut data: Vec<Vec<u8>> = Vec::new();
    for i in 0..data_shards {
        let mut shard = vec![0u8; shard_size];
        for j in 0..shard_size {
            shard[j] = ((i * shard_size + j) & 0xFF) as u8;
        }
        data.push(shard);
    }

    // Encode.
    let data_refs: Vec<&[u8]> = data.iter().map(|s| s.as_slice()).collect();
    let mut parity: Vec<Vec<u8>> = vec![vec![0u8; shard_size]; parity_shards];
    let mut parity_refs: Vec<&mut [u8]> = parity.iter_mut().map(|s| s.as_mut_slice()).collect();
    encode::encode_with_tables(data_shards, parity_shards, &data_refs, &mut parity_refs).unwrap();

    // Verify parity is non-trivial.
    for p in 0..parity_shards {
        assert_ne!(parity[p], data[0], "parity[{p}] should not be trivial copy of data[0]");
    }
    // Go produces: parity[0] = [168, 169, ...], parity[1] = [232, 233, ...]
    println!("parity[0] first 8: {:?}", &parity[0][..8]);
    println!("parity[1] first 8: {:?}", &parity[1][..8]);

    // Decode: lose shard 0 (a data shard).
    let mut present = vec![true; total];
    present[0] = false;

    let mut output_bufs: Vec<Vec<u8>> = Vec::new();
    let mut input_data: Vec<Option<&[u8]>> = Vec::new();
    for i in 0..total {
        let shard_data: &[u8] = if i < data_shards {
            &data[i]
        } else {
            &parity[i - data_shards]
        };
        if i == 0 {
            output_bufs.push(vec![0u8; shard_size]);
            input_data.push(None);
        } else {
            output_bufs.push(shard_data.to_vec());
            input_data.push(Some(shard_data));
        }
    }
    let mut outputs: Vec<&mut [u8]> = output_bufs.iter_mut().map(|b| b.as_mut_slice()).collect();

    decode::reconstruct_with_tables(
        &present,
        &mut outputs,
        &input_data,
        data_shards,
        parity_shards,
        tables,
    ).unwrap();

    // Verify recovered shard 0 matches original data[0].
    assert_eq!(output_bufs[0], data[0], "recovered shard 0 should match original data[0]");
}
