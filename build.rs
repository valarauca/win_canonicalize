fn main() {
    windows::build!(
        Windows::Win32::Foundation::PWSTR,
        Windows::Win32::UI::Shell::PathCchCanonicalizeEx,
        Windows::Win32::System::Com::CoInitialize,
    );
}
