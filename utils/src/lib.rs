pub mod log;
pub mod version;

use chrono::{DateTime, FixedOffset, Local, TimeZone, Utc};

pub fn get_date_time_with_zone(timezone: i32) -> DateTime<FixedOffset> {
    let offset = FixedOffset::east_opt(timezone * 60 * 60).unwrap();
    Utc::now().with_timezone(&offset)
}

pub fn get_localtime_with_increment(increment: i64) -> DateTime<Local> {
    Local::now() + chrono::Duration::seconds(increment)
}

// change timestamp to local time, timestamp is second
pub fn get_local_time_from_timestamp(timestamp: i64) -> DateTime<Local> {
    Local.timestamp_opt(timestamp, 0).unwrap()
}

// get the time difference between now and the timestamp
pub fn get_time_differece_from_now(timestamp: i64) -> i64 {
    let now = Utc::now().timestamp();
    now - timestamp
}

// get random string
pub fn get_random_str(len: usize) -> String {
    use rand::distributions::Alphanumeric;
    use rand::{thread_rng, Rng};

    thread_rng()
        .sample_iter(&Alphanumeric)
        .take(len)
        .map(char::from)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_local_and_naive() {
        // Current local time
        let now = Local::now();
        println!("now: {:?}", now); // 2024-05-30T22:39:17.798456+08:00

        // Current local date
        let today = now.date_naive();
        println!("today: {:?}", today); // 2024-05-30

        let nl = now.naive_local();
        println!("naive local: {:?}", nl); // 2024-05-30T22:39:17.798456

        let nu = now.naive_utc();
        println!("naive utc: {:?}", nu); // 2024-05-30T14:39:17.798456

        // Current local time, converted to `DateTime<FixedOffset>`
        let now_fixed_offset = Local::now().fixed_offset();
        println!("now_fixed_offset: {:?}", now_fixed_offset); // 2024-05-30T22:39:17.798733+08:00
                                                              // or
        let now_fixed_offset: DateTime<FixedOffset> = Local::now().into();
        println!("now_fixed_offset: {:?}", now_fixed_offset); //  2024-05-30T22:39:17.798734+08:00

        // Current time in some timezone (let's use +05:00)
        // Note that it is usually more efficient to use `Utc::now` for this use case.
        let offset = FixedOffset::east_opt(5 * 60 * 60).unwrap();
        let now_with_offset = Local::now().with_timezone(&offset);
        println!("now_with_offset: {:?}", now_with_offset); // 2024-05-30T19:39:17.798735+05:00

        let utc_with_offset = Utc::now().with_timezone(&offset);
        println!("utc_with_offset: {:?}", utc_with_offset); // 2024-05-30T19:39:17.798736+05:00
    }

    #[test]
    fn test_get_local_time_from_timestamp() {
        let ts = 1716877374;
        let time = get_local_time_from_timestamp(1716877374599 / 1000);
        println!("time: {:?}", time);

        println!("local time: {}", Local.timestamp_opt(ts, 0).unwrap());
        println!("utc time: {}", Utc.timestamp_opt(ts, 0).unwrap());
    }
}
