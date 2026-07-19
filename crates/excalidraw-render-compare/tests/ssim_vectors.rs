use excalidraw_render_compare::ssim::{compare_planes, gaussian_kernel_words, SsimError};
use sha2::{Digest, Sha256};

#[test]
fn ssim_uses_the_normative_gaussian_words() {
    let mut bytes = Vec::with_capacity(121 * 8);
    for word in gaussian_kernel_words() {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "b931d31602d633632c7a2f29b24dbe5812a49a8e8a016002b07dc8c0ce93efd3"
    );
}

#[test]
fn ssim_identity_is_exact_for_one_valid_window() {
    let plane = vec![137.0; 11 * 11];
    let score = compare_planes(&plane, &plane, 11, 11).unwrap();
    assert_eq!(score.to_bits(), 0x3ff0_0000_0000_0000);
}

#[test]
fn ssim_rejects_non_finite_inputs_instead_of_masking_them() {
    let first = vec![0.0; 11 * 11];
    let mut second = first.clone();
    second[60] = f64::NAN;
    assert_eq!(
        compare_planes(&first, &second, 11, 11),
        Err(SsimError::NonFinite)
    );

    let finite = vec![f64::MAX; 11 * 11];
    assert_eq!(
        compare_planes(&finite, &finite, 11, 11),
        Err(SsimError::NonFinite)
    );
}

#[test]
fn ssim_requires_complete_valid_windows_and_exact_lengths() {
    assert_eq!(
        compare_planes(&vec![0.0; 10 * 11], &vec![0.0; 10 * 11], 10, 11),
        Err(SsimError::RegionTooSmall)
    );
    assert_eq!(
        compare_planes(&vec![0.0; 120], &vec![0.0; 120], 11, 11),
        Err(SsimError::InvalidBufferLength {
            image: 1,
            expected: 121,
            actual: 120,
        })
    );
}

#[test]
fn ssim_matches_constant_negative_covariance_and_valid_edge_vectors() {
    let black = vec![0.0; 11 * 11];
    let white = vec![255.0; 11 * 11];
    assert_eq!(
        compare_planes(&black, &white, 11, 11).unwrap().to_bits(),
        0x3f1a_3637_230a_f411
    );

    let checkerboard: Vec<f64> = (0..11 * 11)
        .map(|index| {
            if (index / 11 + index % 11) % 2 == 0 {
                0.0
            } else {
                255.0
            }
        })
        .collect();
    let inverse: Vec<f64> = checkerboard.iter().map(|sample| 255.0 - sample).collect();
    assert_eq!(
        compare_planes(&checkerboard, &inverse, 11, 11)
            .unwrap()
            .to_bits(),
        0xbfef_e28f_d163_b621
    );

    let reference = vec![128.0; 12 * 12];
    let mut edge_change = reference.clone();
    edge_change[0] = 129.0;
    assert_eq!(
        compare_planes(&reference, &edge_change, 12, 12)
            .unwrap()
            .to_bits(),
        0x3fef_ffff_fd93_14a9
    );
}

#[test]
fn ssim_matches_every_normative_kernel_position_vector() {
    let expected = [
        0x3feffffff64c52a5,
        0x3fefffffb8502794,
        0x3feffffeac60b1b2,
        0x3feffffbf865ce35,
        0x3feffff826ee3955,
        0x3feffff6331d5df2,
        0x3feffff826ee3955,
        0x3feffffbf865ce35,
        0x3feffffeac60b1b2,
        0x3fefffffb8502794,
        0x3feffffff64c52a5,
        0x3fefffffb8502794,
        0x3feffffdee540a97,
        0x3feffff6331d5df2,
        0x3fefffe23efe7668,
        0x3fefffc617f866b8,
        0x3fefffb7b69c0ccb,
        0x3fefffc617f866b8,
        0x3fefffe23efe7668,
        0x3feffff6331d5df2,
        0x3feffffdee540a97,
        0x3fefffffb8502794,
        0x3feffffeac60b1b2,
        0x3feffff6331d5df2,
        0x3fefffd19df3b2dd,
        0x3fefff737aa822e0,
        0x3feffeef57e518ec,
        0x3feffeac2854ff16,
        0x3feffeef57e518ec,
        0x3fefff737aa822e0,
        0x3fefffd19df3b2dd,
        0x3feffff6331d5df2,
        0x3feffffeac60b1b2,
        0x3feffffbf865ce35,
        0x3fefffe23efe7668,
        0x3fefff737aa822e0,
        0x3feffe589fd6d967,
        0x3feffcd0f8d624d1,
        0x3feffc0c47305fd5,
        0x3feffcd0f8d624d1,
        0x3feffe589fd6d967,
        0x3fefff737aa822e0,
        0x3fefffe23efe7668,
        0x3feffffbf865ce35,
        0x3feffff826ee3955,
        0x3fefffc617f866b8,
        0x3feffeef57e518ec,
        0x3feffcd0f8d624d1,
        0x3feff9f0e74a204a,
        0x3feff8861d1c4ef5,
        0x3feff9f0e74a204a,
        0x3feffcd0f8d624d1,
        0x3feffeef57e518ec,
        0x3fefffc617f866b8,
        0x3feffff826ee3955,
        0x3feffff6331d5df2,
        0x3fefffb7b69c0ccb,
        0x3feffeac2854ff16,
        0x3feffc0c47305fd5,
        0x3feff8861d1c4ef5,
        0x3feff6cdff90b371,
        0x3feff8861d1c4ef5,
        0x3feffc0c47305b76,
        0x3feffeac2854ff16,
        0x3fefffb7b69c0ccb,
        0x3feffff6331d5df2,
        0x3feffff826ee3955,
        0x3fefffc617f866b8,
        0x3feffeef57e516bd,
        0x3feffcd0f8d624d1,
        0x3feff9f0e74a1e1b,
        0x3feff8861d1c4ef5,
        0x3feff9f0e74a1e1b,
        0x3feffcd0f8d624d1,
        0x3feffeef57e516bd,
        0x3fefffc617f866b8,
        0x3feffff826ee3955,
        0x3feffffbf865ce35,
        0x3fefffe23efe7897,
        0x3fefff737aa822e0,
        0x3feffe589fd6d967,
        0x3feffcd0f8d624d1,
        0x3feffc0c47305b76,
        0x3feffcd0f8d624d1,
        0x3feffe589fd6d967,
        0x3fefff737aa822e0,
        0x3fefffe23efe7897,
        0x3feffffbf865ce35,
        0x3feffffeac60b1b2,
        0x3feffff6331d5df2,
        0x3fefffd19df3b73e,
        0x3fefff737aa822e0,
        0x3feffeef57e516bd,
        0x3feffeac2854ff16,
        0x3feffeef57e516bd,
        0x3fefff737aa822e0,
        0x3fefffd19df3b73e,
        0x3feffff6331d5df2,
        0x3feffffeac60b1b2,
        0x3fefffffb8502794,
        0x3feffffdee540a97,
        0x3feffff6331d5df2,
        0x3fefffe23efe7897,
        0x3fefffc617f866b8,
        0x3fefffb7b69c0ccb,
        0x3fefffc617f866b8,
        0x3fefffe23efe7897,
        0x3feffff6331d5df2,
        0x3feffffdee540a97,
        0x3fefffffb8502794,
        0x3feffffff64c52a5,
        0x3fefffffb8502794,
        0x3feffffeac60b1b2,
        0x3feffffbf865ce35,
        0x3feffff826ee3955,
        0x3feffff6331d5df2,
        0x3feffff826ee3955,
        0x3feffffbf865ce35,
        0x3feffffeac60b1b2,
        0x3fefffffb8502794,
        0x3feffffff64c52a5,
    ];
    let reference = vec![128.0; 11 * 11];
    let mut actual = Vec::with_capacity(121);
    for position in 0..121 {
        let mut candidate = reference.clone();
        candidate[position] = 129.0;
        actual.push(
            compare_planes(&reference, &candidate, 11, 11)
                .unwrap()
                .to_bits(),
        );
    }
    assert_eq!(actual, expected);

    let bytes: Vec<u8> = actual.iter().flat_map(|word| word.to_le_bytes()).collect();
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        "5707c05ca69429cbe1345e7fe3062444bd32614188dbcc4f9f3d6c5e574d45f1"
    );
}
