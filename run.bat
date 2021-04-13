@echo off

echo ==================== Rust Compiler Information ====================

rustc --version --verbose

echo ==================== Compiling Bootloader ====================

cargo build --package lightsaber_bootloader --target x86_64-unknown-uefi -Z build-std=core,alloc --verbose

echo ==================== Compiling System Kernel ====================

cargo build --target .\x86_64-unknown-lightsaber.json -Z build-std=core,alloc --verbose

echo ==================== Writing Built Files ====================

xcopy .\target\x86_64-unknown-uefi\debug\lightsaber_bootloader.efi .\build\efi\boot\lightsaber_bootloader.efi
xcopy .\target\x86_64-unknown-lightsaber\debug\project_lightsaber .\build\efi\kernel\lightsaber.elf
