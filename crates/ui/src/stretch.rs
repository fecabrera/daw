use super::*;

// Rounded ratio scaling uses u128 so long timeline ranges cannot overflow.
fn scaled_length(primary_length: u64, clip_length: u64, original_primary: u64) -> u128 {
    let product = u128::from(primary_length) * u128::from(clip_length);
    let divisor = u128::from(original_primary);
    product / divisor + u128::from(product % divisor >= divisor.div_ceil(2))
}

fn length_bounds(clip: &Clip, left: bool) -> (u64, u64) {
    let stretch = clip.stretch.unwrap_or(daw_core::ClipStretch {
        source_frames: 1,
        output_frames: 1,
    });
    let numerator = u128::from(clip.length_frames) * u128::from(stretch.source_frames);
    let denominator = u128::from(stretch.output_frames);
    let minimum = numerator.div_ceil(denominator * 8).max(1) as u64;
    let maximum = numerator.saturating_mul(8) / denominator;
    let timeline_limit = if left {
        clip.end()
    } else {
        u64::MAX - clip.start_frame
    };
    (minimum, maximum.min(u128::from(timeline_limit)) as u64)
}

// Find the primary duration limits that keep every rounded group duration valid.
fn primary_bounds(primary: &Clip, clips: &[SelectedClip], left: bool) -> Option<(u64, u64)> {
    let (mut minimum, mut maximum) = length_bounds(primary, left);
    for entry in clips {
        let (lower, upper) = length_bounds(&entry.clip, left);
        let scale = |length| scaled_length(length, entry.clip.length_frames, primary.length_frames);
        let (mut lo, mut hi) = (minimum, maximum);
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if scale(mid) < u128::from(lower) {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        minimum = lo;
        let (mut lo, mut hi) = (minimum, maximum);
        while lo < hi {
            let mid = lo + (hi - lo).div_ceil(2);
            if scale(mid) > u128::from(upper) {
                hi = mid - 1;
            } else {
                lo = mid;
            }
        }
        maximum = lo;
        if minimum > maximum
            || scale(minimum) < u128::from(lower)
            || scale(maximum) > u128::from(upper)
        {
            return None;
        }
    }
    Some((minimum, maximum))
}

impl DawUi {
    pub(super) fn stretch_group_preview(
        &self,
        drag: &Drag,
        pointer: Pos2,
        unsnapped: bool,
    ) -> Vec<ClipPreview> {
        let left = drag.mode == ClipDragMode::StretchLeft;
        let single = [SelectedClip {
            clip: drag.clip.clone(),
            track: drag.track,
            track_index: self
                .session
                .project
                .tracks
                .iter()
                .position(|track| track.id == drag.track)
                .unwrap_or(0),
        }];
        let clips = if drag.clips.is_empty() {
            &single[..]
        } else {
            &drag.clips
        };
        let Some((minimum, maximum)) = primary_bounds(&drag.clip, clips, left) else {
            return Vec::new();
        };
        let delta = i128::from(self.drag_delta(drag, pointer));
        let primary_length = if left {
            let end = drag.clip.end();
            end - self.clip_edge_frame(
                drag,
                i128::from(drag.clip.start_frame) + delta,
                end - maximum..=end - minimum,
                end,
                unsnapped || delta == 0,
            )
        } else {
            self.clip_edge_frame(
                drag,
                i128::from(drag.clip.end()) + delta,
                drag.clip.start_frame + minimum..=drag.clip.start_frame + maximum,
                drag.clip.start_frame,
                unsnapped || delta == 0,
            ) - drag.clip.start_frame
        };
        let mut valid = true;
        let previews: Vec<_> = clips
            .iter()
            .map(|entry| {
                let candidate = (|| {
                    let length = u64::try_from(scaled_length(
                        primary_length,
                        entry.clip.length_frames,
                        drag.clip.length_frames,
                    ))
                    .ok()?;
                    let start = if left {
                        entry.clip.end().checked_sub(length)?
                    } else {
                        entry.clip.start_frame
                    };
                    let mut clip = entry.clip.stretched_to(start, length)?;
                    let source = self
                        .session
                        .project
                        .assets
                        .iter()
                        .find(|asset| asset.id == clip.asset_id)?;
                    let available = clip.source_length(source.decoded_frame_count)?;
                    let used = clip.repeat.map_or(length, |repeat| repeat.length_frames);
                    // Offset and repeat-base rounding can exceed the asset by one frame.
                    clip.source_offset_frame =
                        clip.source_offset_frame.min(available.checked_sub(used)?);
                    Some(clip)
                })();
                valid &= candidate.is_some();
                ClipPreview {
                    clip: candidate.unwrap_or_else(|| entry.clip.clone()),
                    track: entry.track,
                    track_index: entry.track_index,
                    valid: true,
                    removed: false,
                }
            })
            .collect();
        previews
            .into_iter()
            .map(|mut preview| {
                preview.valid = valid;
                preview
            })
            .collect()
    }

    pub(super) fn finish_clip_stretch(&mut self, drag: &Drag, pointer: Pos2, unsnapped: bool) {
        let Some(edge) = drag.mode.stretch_edge() else {
            return;
        };
        let previews = self.stretch_group_preview(drag, pointer, unsnapped);
        if previews.is_empty() || previews.iter().any(|preview| !preview.valid) {
            return;
        }
        if previews.iter().all(|preview| {
            self.session.project.tracks.iter().any(|track| {
                track.clips.iter().any(|clip| {
                    clip.id == preview.clip.id
                        && (clip.start_frame, clip.length_frames)
                            == (preview.clip.start_frame, preview.clip.length_frames)
                })
            })
        }) {
            return;
        }
        let clips: Vec<_> = previews.into_iter().map(|preview| preview.clip).collect();
        let next = match self.session.project.stretched_clips(&clips, edge) {
            Ok(next) => next,
            Err(error) => {
                self.fail(error);
                return;
            }
        };
        let mut session = self.session.clone();
        session.project = next;
        self.run_job("Stretching clips", move || {
            session
                .prepare_stretches()
                .map_err(|error| error.to_string())?;
            Ok(Job::Loaded(session, true))
        });
    }
}
