// Synthetic Dirac container from EXTERNAL_MEASUREMENTS.md, not program code.
pub fn dirac_project(fs: u32) -> Vec<u8> {
    fn var(mut n: u64) -> Vec<u8> {
        let mut out = Vec::new();
        while n >= 128 {
            out.push((n as u8 & 127) | 128);
            n >>= 7;
        }
        out.push(n as u8);
        out
    }
    fn int(field: u64, n: u64) -> Vec<u8> {
        [var(field << 3), var(n)].concat()
    }
    fn bytes(field: u64, data: &[u8]) -> Vec<u8> {
        [var(field << 3 | 2), var(data.len() as u64), data.to_vec()].concat()
    }
    let mut matrix = [int(1, fs.into()), int(2, 3), int(3, 2)].concat();
    for _ in 0..6 {
        let codes: Vec<u8> = (0..8192)
            .flat_map(|i| {
                let code: i16 = if i == 100 {
                    30000
                } else if i > 100 {
                    (12000.0 * (-((i - 100) as f64) / 450.0).exp() * (i as f64 * 0.2).cos()) as i16
                } else {
                    0
                };
                code.to_be_bytes()
            })
            .collect();
        let element = [
            int(1, fs.into()),
            var(5 << 3 | 5),
            1_f32.to_le_bytes().to_vec(),
            bytes(6, &codes),
        ]
        .concat();
        matrix.extend(bytes(4, &element));
    }
    let frequency: Vec<u8> = [10_f32, 100., 1000., 10000., 24000.]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let gains: Vec<u8> = [-2_f32, -1., 0., 1., 0.]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    let mut message = [
        int(3, fs.into()),
        bytes(12, &matrix),
        bytes(14, &frequency),
        bytes(15, &gains),
        bytes(24, "Test microphone".as_bytes()),
    ]
    .concat();
    for (name, kind) in [("Left", 1), ("Right", 1), ("Subwoofer", 4)] {
        message.extend(bytes(
            30,
            &[bytes(1, name.as_bytes()), int(2, kind)].concat(),
        ));
    }
    let title: Vec<u8> = "Test room"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect();
    [
        10_u32.to_be_bytes().to_vec(),
        (title.len() as u32).to_be_bytes().to_vec(),
        title,
        (message.len() as u32).to_be_bytes().to_vec(),
        message,
    ]
    .concat()
}
