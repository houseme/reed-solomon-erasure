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
