//! A read-only summary of active transfers, derived from their owning feature state.
use super::instances::{InstanceRuntime, InstancesState};

#[derive(Debug)]
pub struct DownloadSummary {
    pub active_count: usize,
    pub profile_name: String,
    pub task: Option<String>,
    /// Progress of the current file, never interpreted as overall installation progress.
    pub current_file: Option<(u64, Option<u64>)>,
}

impl DownloadSummary {
    pub fn from_instances(instances: &InstancesState) -> Option<Self> {
        let mut active = instances.profiles().iter().filter_map(|profile| {
            let (task, current_file) = match instances.runtime(&profile.id) {
                InstanceRuntime::Installing {
                    task,
                    received,
                    total,
                    ..
                } => (task, Some((received, total.filter(|total| *total > 0)))),
                InstanceRuntime::DownloadingMod { name } => (Some(name), None),
                _ => return None,
            };
            Some(Self {
                active_count: 1,
                profile_name: profile.name.clone(),
                task,
                current_file,
            })
        });
        let mut summary = active.next()?;
        summary.active_count += active.count();
        if summary.active_count > 1 {
            summary.current_file = None;
            summary.task = None;
        }
        Some(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{instances::InstanceProfile, versions::LoaderChoice};

    fn state() -> InstancesState {
        InstancesState::from_profiles(vec![
            InstanceProfile::new(
                "First".into(),
                "1.21".into(),
                LoaderChoice::Vanilla,
                None,
                Default::default(),
            ),
            InstanceProfile::new(
                "Second".into(),
                "1.21".into(),
                LoaderChoice::Vanilla,
                None,
                Default::default(),
            ),
        ])
    }

    #[test]
    fn tracks_active_jobs_and_clears_after_success_or_failure() {
        let mut state = state();
        let first = state.profiles()[0].id;
        let second = state.profiles()[1].id;
        assert!(DownloadSummary::from_instances(&state).is_none());
        state.mark_installing(first);
        state.update_progress(first, None, None, Some((25, Some(100))), false);
        let summary = DownloadSummary::from_instances(&state).unwrap();
        assert_eq!(summary.current_file, Some((25, Some(100))));
        state.mark_mod_downloading(second, "Sodium".into());
        let summary = DownloadSummary::from_instances(&state).unwrap();
        assert_eq!(summary.active_count, 2);
        assert_eq!(summary.current_file, None);
        state.mark_installed(first, "1.21".into());
        let summary = DownloadSummary::from_instances(&state).unwrap();
        assert_eq!(summary.active_count, 1);
        assert_eq!(summary.task.as_deref(), Some("Sodium"));
        assert_eq!(summary.current_file, None);
        state.mark_mod_download_failed(second, "Offline".into());
        assert!(DownloadSummary::from_instances(&state).is_none());
    }

    #[test]
    fn unknown_size_and_new_tasks_do_not_reuse_file_progress() {
        let mut state = state();
        let id = state.profiles()[0].id;
        state.mark_installing(id);
        state.update_progress(id, None, None, Some((25, Some(0))), false);
        assert_eq!(
            DownloadSummary::from_instances(&state)
                .unwrap()
                .current_file,
            Some((25, None))
        );
        state.update_progress(id, None, Some("Next file".into()), None, false);
        assert_eq!(
            DownloadSummary::from_instances(&state)
                .unwrap()
                .current_file,
            Some((0, None))
        );
        state.mark_failed(id, "Offline".into());
        assert!(DownloadSummary::from_instances(&state).is_none());
    }
}
