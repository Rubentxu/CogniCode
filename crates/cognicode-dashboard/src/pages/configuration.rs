//! Configuration page for project settings

use leptos::prelude::*;
use crate::components::Shell;

#[component]
pub fn ConfigurationPage() -> impl IntoView {
    let rule_profiles = vec![
        ("sonarqube", "SonarQube Default"),
        ("security-first", "Security First"),
        ("minimal", "Minimal Rules"),
        ("strict", "Strict Mode"),
    ];

    let quality_gates = vec![
        ("sonarqube-way", "SonarQube Way"),
        ("sonarqube-way-strict", "SonarQube Way - Strict"),
        ("security-defaults", "Security Defaults"),
        ("production-prevents", "Production Prevents"),
    ];

    view! {
        <Shell>
            <div style="max-width: 800px; margin: 0 auto;">
                <header style="margin-bottom: 48px;">
                    <h1 class="text-h1">Configuration</h1>
                    <p style="margin-top: 8px; color: var(--color-text-secondary);">
                        Configure project analysis settings and quality gate preferences
                    </p>
                </header>

                <form on:submit=|_e| { /* prevent default */ }>
                    <div style="display: flex; flex-direction: column; gap: 32px;">
                        <section class="card">
                            <h2 class="text-h3" style="margin-bottom: 24px;">Project Settings</h2>

                            <div style="display: flex; flex-direction: column; gap: 20px;">
                                <div>
                                    <label for="project-path" style="display: block; font-size: 14px; font-weight: 500; margin-bottom: 8px; color: var(--color-text-primary);">
                                        Project Path
                                    </label>
                                    <input
                                        id="project-path"
                                        type="text"
                                        class="input"
                                        value="/home/user/project"
                                        placeholder="Enter the path to your project directory"
                                        style="width: 100%;"
                                    />
                                    <p style="margin-top: 8px; font-size: 13px; color: var(--color-text-muted);">
                                        Absolute path to the project root directory
                                    </p>
                                </div>

                                <div>
                                    <label for="project-name" style="display: block; font-size: 14px; font-weight: 500; margin-bottom: 8px; color: var(--color-text-primary);">
                                        Project Name
                                    </label>
                                    <input
                                        id="project-name"
                                        type="text"
                                        class="input"
                                        value="My Project"
                                        placeholder="Enter a name for this project"
                                        style="width: 100%;"
                                    />
                                </div>
                            </div>
                        </section>

                        <section class="card">
                            <h2 class="text-h3" style="margin-bottom: 24px;">Analysis Rules</h2>

                            <div style="display: flex; flex-direction: column; gap: 20px;">
                                <div>
                                    <label for="rule-profile" style="display: block; font-size: 14px; font-weight: 500; margin-bottom: 8px; color: var(--color-text-primary);">
                                        Rule Profile
                                    </label>
                                    <select id="rule-profile" class="input select" style="width: 100%;">
                                        {rule_profiles.iter().map(|(value, label)| {
                                            let is_selected = *value == "sonarqube";
                                            let val = *value;
                                            let lbl = *label;
                                            view! {
                                                <option value={val} selected={is_selected}>
                                                    {lbl}
                                                </option>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </select>
                                    <p style="margin-top: 8px; font-size: 13px; color: var(--color-text-muted);">
                                        Determines which rules are applied during analysis
                                    </p>
                                </div>

                                <div>
                                    <label style="display: flex; align-items: center; gap: 12px; cursor: pointer;">
                                        <input type="checkbox" checked={true} />
                                        <span style="font-size: 14px; font-weight: 500; color: var(--color-text-primary);">
                                            Include test files in analysis
                                        </span>
                                    </label>
                                </div>

                                <div>
                                    <label style="display: flex; align-items: center; gap: 12px; cursor: pointer;">
                                        <input type="checkbox" checked={true} />
                                        <span style="font-size: 14px; font-weight: 500; color: var(--color-text-primary);">
                                            Analyze dependencies for known vulnerabilities
                                        </span>
                                    </label>
                                </div>
                            </div>
                        </section>

                        <section class="card">
                            <h2 class="text-h3" style="margin-bottom: 24px;">Quality Gate</h2>

                            <div style="display: flex; flex-direction: column; gap: 20px;">
                                <div>
                                    <label for="quality-gate" style="display: block; font-size: 14px; font-weight: 500; margin-bottom: 8px; color: var(--color-text-primary);">
                                        Default Quality Gate
                                    </label>
                                    <select id="quality-gate" class="input select" style="width: 100%;">
                                        {quality_gates.iter().map(|(value, label)| {
                                            let is_selected = *value == "sonarqube-way";
                                            let val = *value;
                                            let lbl = *label;
                                            view! {
                                                <option value={val} selected={is_selected}>
                                                    {lbl}
                                                </option>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </select>
                                    <p style="margin-top: 8px; font-size: 13px; color: var(--color-text-muted);">
                                        The quality gate used to determine build success or failure
                                    </p>
                                </div>

                                <div>
                                    <label style="display: flex; align-items: center; gap: 12px; cursor: pointer;">
                                        <input type="checkbox" checked={true} />
                                        <span style="font-size: 14px; font-weight: 500; color: var(--color-text-primary);">
                                            Fail build on quality gate failure
                                        </span>
                                    </label>
                                </div>

                                <div>
                                    <label style="display: flex; align-items: center; gap: 12px; cursor: pointer;">
                                        <input type="checkbox" checked={false} />
                                        <span style="font-size: 14px; font-weight: 500; color: var(--color-text-primary);">
                                            Block deployment on gate failure
                                        </span>
                                    </label>
                                </div>
                            </div>
                        </section>

                        <section class="card">
                            <h2 class="text-h3" style="margin-bottom: 24px;">Notifications</h2>

                            <div style="display: flex; flex-direction: column; gap: 16px;">
                                <div>
                                    <label style="display: flex; align-items: center; gap: 12px; cursor: pointer;">
                                        <input type="checkbox" checked={true} />
                                        <span style="font-size: 14px; font-weight: 500; color: var(--color-text-primary);">
                                            Notify on analysis completion
                                        </span>
                                    </label>
                                </div>

                                <div>
                                    <label style="display: flex; align-items: center; gap: 12px; cursor: pointer;">
                                        <input type="checkbox" checked={false} />
                                        <span style="font-size: 14px; font-weight: 500; color: var(--color-text-primary);">
                                            Alert when quality gate fails
                                        </span>
                                    </label>
                                </div>

                                <div>
                                    <label style="display: flex; align-items: center; gap: 12px; cursor: pointer;">
                                        <input type="checkbox" checked={true} />
                                        <span style="font-size: 14px; font-weight: 500; color: var(--color-text-primary);">
                                            Weekly summary report
                                        </span>
                                    </label>
                                </div>
                            </div>
                        </section>

                        <section style="display: flex; justify-content: flex-end; gap: 16px; padding-top: 16px; border-top: 1px solid var(--color-border);">
                            <button type="button" class="btn btn-secondary">
                                Cancel
                            </button>
                            <button type="submit" class="btn btn-primary">
                                Save Configuration
                            </button>
                        </section>
                    </div>
                </form>
            </div>
        </Shell>
    }.into_view()
}
