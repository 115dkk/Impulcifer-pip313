#![forbid(unsafe_code)]
use impulcifer_dsp::filters::butter_bandpass;
#[test]
fn room_tuning2_bandpass_matches_scipy() {
    let cases = [
        (
            2,
            48.5,
            51.5,
            600.,
            vec![
                [
                    0.00024135904904198086,
                    0.0004827180980839617,
                    0.00024135904904198086,
                    1.,
                    -1.7016786324818156,
                    0.9776123459021749,
                ],
                [1., -2., 1., 1., -1.7243752740560991, 0.9784488509384274],
            ],
        ),
        (
            4,
            500. / 2_f64.sqrt(),
            500. * 2_f64.sqrt(),
            48000.,
            vec![
                [
                    2.700853964318714e-7,
                    5.401707928637427e-7,
                    2.700853964318714e-7,
                    1.,
                    -1.9467977986638063,
                    0.9523568057434735,
                ],
                [1., 2., 1., 1., -1.960782791478935, 0.9639455998661062],
                [1., -2., 1., 1., -1.968945254948772, 0.9770301487236029],
                [1., -2., 1., 1., -1.9856711353336083, 0.9878993191713733],
            ],
        ),
    ];
    for (order, lo, hi, fs, expected) in cases {
        let actual = butter_bandpass(order, lo / (fs / 2.), hi / (fs / 2.));
        assert_eq!(actual.0.len(), expected.len());
        for (a, b) in actual.0.iter().flatten().zip(expected.iter().flatten()) {
            assert!((a - b).abs() < 2e-14, "{a} != {b}");
        }
    }
}
