"""Source fixture directories common to desktop package tests."""
import shutil

NOTICE_DIRS = (
    'crates/unit-ui/assets/fonts', 'crates/ui/assets/icons',
    'crates/ui/assets/licenses', 'crates/unit-ui/assets/licenses',
    'crates/media/assets/licenses', 'crates/oscillator/assets/licenses',
    'crates/equalizer/assets/licenses', 'crates/timestretch/licenses',
    'crates/separation/assets/licenses',
    'crates/plugin-host/assets/licenses', 'crates/plugin-clap/assets/licenses',
    'crates/plugin-vst3/assets/licenses',
)


def stage_notice_sources(source, destination, *, app_icons=False):
    for directory in NOTICE_DIRS:
        shutil.copytree(source / directory, destination / directory)
    if app_icons:
        directory = "crates/ui/assets/app-icon"
        shutil.copytree(source / directory, destination / directory)
