//! Per-save verification reuse. Different retained PCM versions remain independent.
use super::*;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct PreparedVerification<'a> {
    owners: HashMap<(PathBuf, SourceRevision), Vec<&'a AudioData>>,
    #[cfg(test)]
    checks: usize,
}
impl<'a> PreparedVerification<'a> {
    pub fn verify(
        &mut self,
        path: &Path,
        revision: SourceRevision,
        audio: &'a AudioData,
    ) -> Result<()> {
        let owners = self.owners.entry((path.to_owned(), revision)).or_default();
        if owners.iter().any(|old| same_audio(Some(old), Some(audio))) {
            return Ok(());
        }
        #[cfg(test)]
        {
            self.checks += 1;
        }
        if daw_media::verify_prepared_source(path, audio, &|| false).map_err(error)? != revision {
            return Err(error("Project-owned source changed during verification"));
        }
        owners.push(audio);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_pcm_reuses_verification_but_distinct_versions_and_revisions_are_checked() {
        let root = std::env::temp_dir().join(format!("daw-save-verify-{}", Id::new_v4()));
        fs::create_dir(&root).unwrap();
        let path = root.join("source.wav");
        let mut writer = hound::WavWriter::create(
            &path,
            hound::WavSpec {
                channels: 2,
                sample_rate: 48000,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            },
        )
        .unwrap();
        for _ in 0..100 {
            writer.write_sample(0.2f32).unwrap();
            writer.write_sample(-0.1f32).unwrap();
        }
        writer.finalize().unwrap();
        let audio = daw_media::decode_audio(&path).unwrap();
        let shared = audio.clone();
        let revision = SourceRevision::read(&path, &|| false).unwrap();
        let mut cache = PreparedVerification::default();
        for _ in 0..100 {
            cache.verify(&path, revision, &shared).unwrap();
        }
        assert_eq!(cache.checks, 1);
        let mut independent = audio.clone();
        independent.samples = Arc::new(audio.samples.as_ref().clone());
        cache.verify(&path, revision, &independent).unwrap();
        assert_eq!(cache.checks, 2);
        let mut mismatched = audio.clone();
        mismatched.samples = Arc::new(vec![[0.; 2]; audio.samples.len()]);
        assert!(cache.verify(&path, revision, &mismatched).is_err());
        assert_eq!(cache.checks, 3);
        let mut bytes = fs::read(&path).unwrap();
        bytes.extend_from_slice(b"changed encoding");
        fs::write(&path, bytes).unwrap();
        let updated = SourceRevision::read(&path, &|| false).unwrap();
        assert_ne!(revision, updated);
        cache.verify(&path, updated, &shared).unwrap();
        assert_eq!(cache.checks, 4);
        fs::remove_dir_all(root).unwrap();
    }
}
