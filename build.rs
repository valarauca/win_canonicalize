fn main() {
    windows::build!(
        Windows::Win32::Foundation::PWSTR,
        Windows::Win32::UI::Shell::PathCchCanonicalizeEx,
        Windows::Win32::Storage::FileSystem::MoveFileExW,
        Windows::Win32::Storage::FileSystem::MOVE_FILE_FLAGS,
        Windows::Win32::System::Com::CoInitialize
    );
}
