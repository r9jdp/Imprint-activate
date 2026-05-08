import { SectionHeader } from './components/SectionHeader'
import {
  capabilities,
  comparisonCards,
  heroTags,
  pillars,
  workflow,
} from './data/siteContent'

export default function App() {
  return (
    <div className="page-shell">
      <div className="background-haze haze-left" />
      <div className="background-haze haze-right" />

      <header className="topbar">
        <div className="brand">
          <div className="brand-mark">I</div>
          <div className="brand-copy">
            <p className="eyebrow">Imprint</p>
            <p className="brand-subtitle">Local, stateful student agent</p>
          </div>
        </div>

        <nav className="topnav" aria-label="Page">
          <a href="#product">Product</a>
          <a href="#statefulness">State</a>
          <a href="#demo">Demo</a>
          <a className="topnav-cta" href="#download">
            Download
          </a>
        </nav>
      </header>

      <main className="content">
        <section className="hero panel" id="product">
          <div className="hero-copy">
            <p className="eyebrow">Submission Overview</p>
            <h1>Imprint helps students finish real work without losing context.</h1>
            <p className="hero-text">
              It runs locally, works against the real workspace, and keeps enough state to stay useful across
              long-running academic tasks.
            </p>

            <div className="hero-actions">
              <a className="primary-button" href="#download">
                Download Imprint
              </a>
              <a className="secondary-button" href="#demo">
                Demo video
              </a>
            </div>

            <div className="hero-tags" aria-label="Product traits">
              {heroTags.map((tag) => (
                <span key={tag}>{tag}</span>
              ))}
            </div>
          </div>

          <div className="hero-shell">
            <div className="shell-window">
              <div className="shell-tabs">
                <span className="shell-tab active">essay-draft</span>
                <span className="shell-tab">applications</span>
                <span className="shell-tab muted">research</span>
              </div>

              <div className="shell-body">
                <div className="shell-block">
                  <p className="micro-label">Current context</p>
                  <p className="shell-text">
                    Scholarship draft, source tabs, and prior edits are already loaded into the working session.
                  </p>
                </div>

                <div className="shell-block accent">
                  <p className="micro-label">Running action</p>
                  <div className="activity-list">
                    <div className="activity-row">
                      <span className="activity-dot live" />
                      <p>Comparing requirements against the latest draft</p>
                    </div>
                    <div className="activity-row">
                      <span className="activity-dot" />
                      <p>Preparing the next concrete revision</p>
                    </div>
                  </div>
                </div>

                <div className="shell-status">
                  <div>
                    <p className="micro-label">Why it matters</p>
                    <p className="shell-text subdued">
                      The student does not need to restate progress every session.
                    </p>
                  </div>
                  <span className="status-pill">State retained</span>
                </div>
              </div>

              <div className="shell-footer">
                <span>Local machine</span>
                <span>Student workflow</span>
                <span>Execution enabled</span>
              </div>
            </div>
          </div>
        </section>

        <section className="product-grid">
          <article className="panel compact-panel">
            <SectionHeader
              eyebrow="What Imprint Is"
              title="A stateful agent built around student workflows."
            />
            <p className="section-text">
              Imprint is a local, executional desktop or browser agent for coursework, research, writing, forms,
              and applications. It is designed around persistent work, not isolated prompt-response moments.
            </p>
          </article>

          <article className="panel compact-panel">
            <SectionHeader
              eyebrow="What Makes It Special"
              title="Continuity is part of the product."
            />
            <p className="section-text">
              Instead of resetting after every chat turn, Imprint preserves working context so it can resume
              intelligently and help carry multi-step tasks forward.
            </p>
          </article>
        </section>

        <section className="panel">
          <SectionHeader eyebrow="Key Capabilities" title="Built to move work forward." />
          <div className="capability-grid">
            {capabilities.map((item) => (
              <article className="info-card" key={item.title}>
                <div className="card-accent" />
                <h3>{item.title}</h3>
                <p>{item.text}</p>
              </article>
            ))}
          </div>
        </section>

        <section className="state-grid" id="statefulness">
          <article className="panel">
            <SectionHeader
              eyebrow="Why Statefulness Matters"
              title="Student work is fragmented, cumulative, and rarely done in one sitting."
            />
            <p className="section-text">
              A useful student agent needs continuity across interruptions. Statefulness lets Imprint remember what
              has already been done, what matters now, and what should happen next.
            </p>

            <div className="bullet-stack">
              {pillars.map((item) => (
                <div className="bullet-row" key={item}>
                  <span className="bullet-mark" />
                  <p>{item}</p>
                </div>
              ))}
            </div>
          </article>

          <article className="panel comparison-panel">
            {comparisonCards.map((item) => (
              <div className={`comparison-card${item.emphasis ? ' emphasis' : ''}`} key={item.label}>
                <p className="micro-label">{item.label}</p>
                <p className="comparison-text">{item.text}</p>
              </div>
            ))}
          </article>
        </section>

        <section className="panel">
          <SectionHeader eyebrow="How It Works" title="Observe, reason, execute, and carry context forward." />
          <div className="workflow-grid">
            {workflow.map((item) => (
              <article className="workflow-card" key={item.step}>
                <p className="workflow-step">{item.step}</p>
                <h3>{item.title}</h3>
                <p>{item.text}</p>
              </article>
            ))}
          </div>
        </section>

        <section className="panel demo-panel" id="demo">
          <SectionHeader eyebrow="Demo Video" title="Replace this placeholder with the recorded walkthrough." />
          <div className="video-placeholder">
            <div className="video-badge">Demo</div>
            <div className="video-copy">
              <p className="video-title">Embed placeholder</p>
              <p className="video-text">
                Swap this block for an `iframe` or `video` element when the final recording is ready.
              </p>
              <code className="inline-note">frontend/src/App.tsx contains the placeholder markup.</code>
            </div>
          </div>
        </section>

        <section className="panel download-panel" id="download">
          <div>
            <SectionHeader eyebrow="Download" title="Point this to the final submission artifact." />
            <p className="section-text">
              Update the placeholder target once the installer, archive, or hosted build is finalized.
            </p>
          </div>

          <div className="download-actions">
            <a className="primary-button" href="https://example.com/imprint-download">
              Placeholder download link
            </a>
            <p className="cta-note">Change the `href` in `frontend/src/App.tsx` to the real delivery URL.</p>
          </div>
        </section>
      </main>
    </div>
  )
}
