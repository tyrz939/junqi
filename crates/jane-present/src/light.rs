//! Light (PRESENTATION.md §1.7), so far the ambient alone: by hour from the clock's keyframes
//! outdoors, the zone's own light indoors, blue-tinted. The lights, the lightmap and the sun
//! land with PORT.md §7.1 step 5.

/// Ticks in an hour of the clock (60 ticks a second, 2 real minutes an hour).
const HOUR: i32 = 7200;

const NIGHT: [i32; 3] = [58, 66, 116];
const DAWN: [i32; 3] = [230, 190, 190];
const DAY: [i32; 3] = [255, 255, 255];
const SUNSET: [i32; 3] = [255, 196, 150];
const DUSK: [i32; 3] = [150, 120, 170];

/// `(tick of the day, colour)`: linear between them (the TS build's keys, so 17:00 when she
/// steps off the train lands in the sunset).
const KEYS: [(i32, [i32; 3]); 9] = [
    (0, NIGHT),
    (HOUR * 9 / 2, NIGHT),
    (HOUR * 6, DAWN),
    (HOUR * 15 / 2, DAY),
    (HOUR * 33 / 2, DAY),
    (HOUR * 18, SUNSET),
    (HOUR * 39 / 2, DUSK),
    (HOUR * 21, NIGHT),
    (HOUR * 24, NIGHT),
];

/// The ambient per channel (255 is full light) at `clock` ticks since midnight; indoors, the
/// zone's `permille` instead, a little blue.
pub fn ambient(clock: u32, indoor: bool, permille: i16) -> [u8; 3] {
    if indoor {
        let v = i32::from(permille.clamp(0, 1000)) * 255 / 1000;
        return [(v - v / 10) as u8, (v - v / 20) as u8, v as u8];
    }
    let t = (clock % (24 * HOUR as u32)) as i32;
    for w in KEYS.windows(2) {
        let ((t0, c0), (t1, c1)) = (w[0], w[1]);
        if t >= t0 && t <= t1 {
            let span = (t1 - t0).max(1);
            return [0, 1, 2].map(|k| (c0[k] + (c1[k] - c0[k]) * (t - t0) / span) as u8);
        }
    }
    NIGHT.map(|c| c as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_full_and_midnight_is_dark_and_blue() {
        assert_eq!(ambient(12 * 7200, false, 1000), [255; 3]);
        let n = ambient(0, false, 1000);
        assert!(n[2] > n[0] && n[0] < 100, "{n:?}");
        assert_eq!(ambient(23 * 7200, false, 1000), n);
        // 17:00 is on the way to sunset; 20:00 is on the way to night.
        let five = ambient(17 * 7200, false, 1000);
        assert!(five[0] == 255 && five[2] < 255, "{five:?}");
        assert!(ambient(20 * 7200, false, 1000)[0] < five[0]);
    }

    #[test]
    fn indoors_is_the_zone_light() {
        assert_eq!(ambient(0, true, 1000), [230, 243, 255]);
        let dim = ambient(12 * 7200, true, 200);
        assert!(dim[2] < 60, "{dim:?}");
    }
}
