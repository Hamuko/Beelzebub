pub fn clean_name(value: &String) -> &str {
    return value.split('\0').next().unwrap_or(value);
}

#[cfg(test)]
mod tests {
    use test_case::test_case;

    #[test_case("Grand Theft Auto IV", "Grand Theft Auto IV"; "clean name")]
    #[test_case("Rockstar Games Launcher Redirector\0\08\u{12}\u{1}ProductVersion\01.0.0.66\0\0D\0\0Va", "Rockstar Games Launcher Redirector"; "corrupt name")]
    fn clean_name(input: &str, output: &str) {
        assert_eq!(super::clean_name(&String::from(input)), &String::from(output));
    }
}
