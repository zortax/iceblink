//! The calls the driver thread applies.

use crate::thread::command::Command;
use crate::thread::driver::{Flow, Host};

impl Host {
    /// Applies `command`, and returns what the driver does next.
    pub(super) fn apply(&mut self, command: Command) -> Flow {
        let driver = &mut self.driver;
        match command {
            Command::Reserve => self.reserve(),
            Command::Register(reply) => {
                let _ = reply.send(driver.register_client());
            }
            Command::Unregister(client) => driver.unregister_client(client),
            Command::Commit(commit, transaction) => {
                let client = transaction.client();
                driver.commit(commit, transaction);
                self.spares.push(driver.transaction(client));
            }
            Command::CreateOutput {
                config,
                renderer,
                reply,
            } => {
                let output = driver.add_output(config);
                let _ = reply.send(output);
                // The calls queued meanwhile wait for the renderer.
                driver.attach_renderer(output, renderer());
            }
            Command::ConfigureOutput(output, config) => driver.configure_output(output, config),
            Command::SetSubpixelOrder(output, order) => driver.set_subpixel_order(output, order),
            Command::SetRefresh(output, refresh_ns) => driver.set_refresh(output, refresh_ns),
            Command::ReportPresented(output, presented_ns) => {
                driver.report_presented(output, presented_ns);
            }
            Command::DestroyOutput(output) => driver.destroy_output(output),
            Command::RequestFrame(output) => driver.request_frame(output),
            Command::ScrollInput {
                client,
                output,
                gesture,
                predicted,
            } => driver.scroll_input(client, output, gesture, predicted),
            Command::HitTest(output, point, reply) => {
                let _ = reply.send(driver.hit_test(output, point));
            }
            Command::WatchScroll(client, node, watch) => driver.watch_scroll(client, node, watch),
            Command::WatchAnimations(client, on) => driver.watch_animations(client, on),
            Command::WatchFrames(client, output, on) => driver.watch_frames(client, output, on),
            Command::WatchOutput(client, output, watch) => {
                driver.watch_output(client, output, watch);
            }
            Command::ObserveFrames(output, observer) => driver.observe_frames(output, observer),
            Command::WithRenderer(output, task) => driver.with_renderer(output, task),
            Command::ReleaseCachedTargets(output) => driver.release_cached_targets(output),
            Command::ReleaseIdleResources(output) => driver.release_idle_resources(output),
            Command::Trim(to_bytes) => {
                driver.trim(to_bytes);
                // The next publication renews the memory it shrank.
                self.shared.snapshot().memory_ns = 0;
            }
            Command::ForgetCaches => {
                driver.forget_caches();
                self.shared.snapshot().memory_ns = 0;
            }
            Command::SetVerification(on) => driver.set_verification(on),
            Command::SetBudgets(budgets) => driver.set_budgets(budgets),
            Command::Step(clock) => {
                driver.step(clock);
                return Flow::Publish;
            }
            Command::Sync(reply) => {
                // The answer waits for what the calls before it published.
                self.shared.snapshot().memory_ns = 0;
                self.publish();
                let _ = reply.send(());
            }
            Command::Shutdown => return Flow::Stop,
        }
        Flow::Continue
    }
}
