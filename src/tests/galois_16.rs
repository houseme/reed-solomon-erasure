extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;

use super::{fill_random, option_shards_into_shards, shards_into_option_shards};
use crate::galois_16::ReedSolomon;

const QUICKCHECK_MAX_SHARD_LEN: usize = 256;

fn quickcheck_shard_len(size: usize) -> usize {
    1 + size % QUICKCHECK_MAX_SHARD_LEN
}

fn qc_params(data: usize, parity: usize) -> (usize, usize) {
    let data = 1 + data % 31;
    let mut parity = 1 + parity % 31;
    if data + parity > 32 {
        parity -= data + parity - 32;
    }
    (data, parity)
}

fn gen_corrupt_positions(n: usize, total: usize) -> Vec<usize> {
    let n = n.min(total);
    let mut positions: Vec<usize> = (0..total).collect();
    for i in 0..n {
        let j = rand::random_range(i..total);
        positions.swap(i, j);
    }
    positions.truncate(n);
    positions
}

macro_rules! make_random_shards {
    ($per_shard:expr, $size:expr) => {{
        let mut shards = Vec::with_capacity($size);
        for _ in 0..$size {
            shards.push(vec![[0; 2]; $per_shard]);
        }

        for s in shards.iter_mut() {
            fill_random(s);
        }

        shards
    }};
}

#[test]
fn correct_field_order_restriction() {
    const ORDER: usize = 1 << 16;

    assert!(ReedSolomon::new(ORDER, 1).is_err());
    assert!(ReedSolomon::new(1, ORDER).is_err());

    // way too slow, because it needs to build a 65536*65536 vandermonde matrix
    // assert!(ReedSolomon::new(ORDER - 1, 1).is_ok());
    assert!(ReedSolomon::new(1, ORDER - 1).is_ok());
}

quickcheck! {
    fn qc_encode_verify_reconstruct_verify(data: usize,
                                           parity: usize,
                                           corrupt: usize,
                                           size: usize) -> bool {
        let (data, parity) = qc_params(data, parity);
        let corrupt = corrupt % (parity + 1);
        let corrupt_pos_s = gen_corrupt_positions(corrupt, data + parity);
        let size = quickcheck_shard_len(size);

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        {
            let mut refs =
                convert_2D_slices!(expect =>to_mut_vec &mut [[u8; 2]]);

            r.encode(&mut refs).unwrap();
        }

        let expect = expect;

        let mut shards = expect.clone();

        // corrupt shards
        for &p in corrupt_pos_s.iter() {
            fill_random(&mut shards[p]);
        }
        let mut slice_present = vec![true; data + parity];
        for &p in corrupt_pos_s.iter() {
            slice_present[p] = false;
        }

        // reconstruct
        {
            let mut refs: Vec<_> = shards.iter_mut()
                .map(|i| &mut i[..])
                .zip(slice_present.iter().cloned())
                .collect();

            r.reconstruct(&mut refs[..]).unwrap();
        }

        ({
            let refs =
                convert_2D_slices!(expect =>to_vec &[[u8; 2]]);

            r.verify(&refs).unwrap()
        })
            &&
            expect == shards
            &&
            ({
                let refs =
                    convert_2D_slices!(shards =>to_vec &[[u8; 2]]);

                r.verify(&refs).unwrap()
            })
    }

    fn qc_encode_verify_reconstruct_verify_shards(data: usize,
                                                  parity: usize,
                                                  corrupt: usize,
                                                  size: usize) -> bool {
        let (data, parity) = qc_params(data, parity);
        let corrupt = corrupt % (parity + 1);
        let corrupt_pos_s = gen_corrupt_positions(corrupt, data + parity);
        let size = quickcheck_shard_len(size);

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        r.encode(&mut expect).unwrap();

        let expect = expect;

        let mut shards = shards_into_option_shards(expect.clone());

        // corrupt shards
        for &p in corrupt_pos_s.iter() {
            shards[p] = None;
        }

        // reconstruct
        r.reconstruct(&mut shards).unwrap();

        let shards = option_shards_into_shards(shards);

        r.verify(&expect).unwrap()
            && expect == shards
            && r.verify(&shards).unwrap()
    }

    fn qc_verify(data: usize,
                 parity: usize,
                 corrupt: usize,
                 size: usize) -> bool {
        let (data, parity) = qc_params(data, parity);
        let corrupt = corrupt % (parity + 1);
        let corrupt_pos_s = gen_corrupt_positions(corrupt, data + parity);
        let size = quickcheck_shard_len(size);

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        {
            let mut refs =
                convert_2D_slices!(expect =>to_mut_vec &mut [[u8; 2]]);

            r.encode(&mut refs).unwrap();
        }

        let expect = expect;

        let mut shards = expect.clone();

        // corrupt shards
        for &p in corrupt_pos_s.iter() {
            fill_random(&mut shards[p]);
        }

        ({
            let refs =
                convert_2D_slices!(expect =>to_vec &[[u8; 2]]);

            r.verify(&refs).unwrap()
        })
            &&
            ((corrupt > 0 && expect != shards)
             || (corrupt == 0 && expect == shards))
            &&
            ({
                let refs =
                    convert_2D_slices!(shards =>to_vec &[[u8; 2]]);

                (corrupt > 0 && !r.verify(&refs).unwrap())
                    || (corrupt == 0 && r.verify(&refs).unwrap())
            })
    }

    fn qc_verify_shards(data: usize,
                        parity: usize,
                        corrupt: usize,
                        size: usize) -> bool {
        let (data, parity) = qc_params(data, parity);
        let corrupt = corrupt % (parity + 1);
        let corrupt_pos_s = gen_corrupt_positions(corrupt, data + parity);
        let size = quickcheck_shard_len(size);

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        r.encode(&mut expect).unwrap();

        let expect = expect;

        let mut shards = expect.clone();

        // corrupt shards
        for &p in corrupt_pos_s.iter() {
            fill_random(&mut shards[p]);
        }

        r.verify(&expect).unwrap()
            &&
            ((corrupt > 0 && expect != shards)
             || (corrupt == 0 && expect == shards))
            &&
            ((corrupt > 0 && !r.verify(&shards).unwrap())
             || (corrupt == 0 && r.verify(&shards).unwrap()))
    }

    fn qc_encode_sep_same_as_encode(data: usize,
                                    parity: usize,
                                    size: usize) -> bool {
        let (data, parity) = qc_params(data, parity);
        let size = quickcheck_shard_len(size);

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        {
            let mut refs =
                convert_2D_slices!(expect =>to_mut_vec &mut [[u8; 2]]);

            r.encode(&mut refs).unwrap();
        }

        let expect = expect;

        {
            let (data, parity) = shards.split_at_mut(data);

            let data_refs =
                convert_2D_slices!(data =>to_mut_vec &[[u8; 2]]);

            let mut parity_refs =
                convert_2D_slices!(parity =>to_mut_vec &mut [[u8; 2]]);

            r.encode_sep(&data_refs, &mut parity_refs).unwrap();
        }

        let shards = shards;

        expect == shards
    }

    fn qc_encode_sep_same_as_encode_shards(data: usize,
                                           parity: usize,
                                           size: usize) -> bool {
        let (data, parity) = qc_params(data, parity);
        let size = quickcheck_shard_len(size);

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        r.encode(&mut expect).unwrap();

        let expect = expect;

        {
            let (data, parity) = shards.split_at_mut(data);

            r.encode_sep(data, parity).unwrap();
        }

        let shards = shards;

        expect == shards
    }

    fn qc_encode_single_same_as_encode(data: usize,
                                       parity: usize,
                                       size: usize) -> bool {
        let (data, parity) = qc_params(data, parity);
        let size = quickcheck_shard_len(size);

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        {
            let mut refs =
                convert_2D_slices!(expect =>to_mut_vec &mut [[u8; 2]]);

            r.encode(&mut refs).unwrap();
        }

        let expect = expect;

        {
            let mut refs =
                convert_2D_slices!(shards =>to_mut_vec &mut [[u8; 2]]);

            for i in 0..data {
                r.encode_single(i, &mut refs).unwrap();
            }
        }

        let shards = shards;

        expect == shards
    }

    fn qc_encode_single_same_as_encode_shards(data: usize,
                                              parity: usize,
                                              size: usize) -> bool {
        let (data, parity) = qc_params(data, parity);
        let size = quickcheck_shard_len(size);

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        r.encode(&mut expect).unwrap();

        let expect = expect;

        for i in 0..data {
            r.encode_single(i, &mut shards).unwrap();
        }

        let shards = shards;

        expect == shards
    }

    fn qc_encode_single_sep_same_as_encode(data: usize,
                                           parity: usize,
                                           size: usize) -> bool {
        let (data, parity) = qc_params(data, parity);
        let size = quickcheck_shard_len(size);

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        {
            let mut refs =
                convert_2D_slices!(expect =>to_mut_vec &mut [[u8; 2]]);

            r.encode(&mut refs).unwrap();
        }

        let expect = expect;

        {
            let (data_shards, parity_shards) = shards.split_at_mut(data);

            let data_refs =
                convert_2D_slices!(data_shards =>to_mut_vec &[[u8; 2]]);

            let mut parity_refs =
                convert_2D_slices!(parity_shards =>to_mut_vec &mut [[u8; 2]]);

            for (i, shard) in data_refs.iter().enumerate().take(data) {
                r.encode_single_sep(i, shard, &mut parity_refs).unwrap();
            }
        }

        let shards = shards;

        expect == shards
    }

    fn qc_encode_single_sep_same_as_encode_shards(data: usize,
                                                  parity: usize,
                                                  size: usize) -> bool {
        let (data, parity) = qc_params(data, parity);
        let size = quickcheck_shard_len(size);

        let r = ReedSolomon::new(data, parity).unwrap();

        let mut expect = make_random_shards!(size, data + parity);
        let mut shards = expect.clone();

        r.encode(&mut expect).unwrap();

        let expect = expect;

        {
            let (data_shards, parity_shards) = shards.split_at_mut(data);

            for (i, shard) in data_shards.iter().enumerate().take(data) {
                r.encode_single_sep(i, shard, parity_shards).unwrap();
            }
        }

        let shards = shards;

        expect == shards
    }
}
