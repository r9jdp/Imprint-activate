import { useEffect } from 'react'
import { SectionHeader } from './components/SectionHeader'
import {
  capabilities,
  comparisonCards,
  heroTags,
  pillars,
  workflow,
} from './data/siteContent'

const heroSignals = [
  {
    label: 'Real working context',
    text: 'Files, browser state, and active task stay inside the loop.',
  },
  {
    label: 'Built for ongoing work',
    text: 'Useful across assignments, essays, applications, and repeated admin.',
  },
] as const

export default function App() {
  useEffect(() => {
    const elements = Array.from(document.querySelectorAll<HTMLElement>('[data-reveal]'))
    const observer = new IntersectionObserver(
      (entries) => {
        entries.forEach((entry) => {
          if (entry.isIntersecting) {
            entry.target.classList.add('is-visible')
            observer.unobserve(entry.target)
          }
        })
      },
      { threshold: 0.16, rootMargin: '0px 0px -8% 0px' },
    )

    elements.forEach((element) => observer.observe(element))

    return () => observer.disconnect()
  }, [])

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
          <a href="#demo">Demo</a>
          <a href="#statefulness">State</a>
          <a className="topnav-cta" href="#download">
            Download
          </a>
        </nav>
      </header>

      <main className="content">
        <section className="hero-stage panel" id="product" data-reveal>
          <div className="hero-copy">
            <p className="eyebrow intro-line intro-1">Submission Overview</p>
            <h1 className="intro-line intro-2">Imprint helps students keep momentum across real work.</h1>
            <p className="hero-text intro-line intro-3">
              A local-first agent that acts on the real machine, keeps working state between sessions, and
              supports long-running academic tasks instead of isolated prompts.
            </p>

            <div className="hero-actions intro-line intro-4">
              <a className="primary-button" href="#download">
                Download Imprint
              </a>
              <a className="secondary-button" href="#demo">
                Watch demo
              </a>
            </div>

            <div className="hero-tags intro-line intro-5" aria-label="Product traits">
              {heroTags.map((tag) => (
                <span key={tag}>{tag}</span>
              ))}
            </div>

            <div className="hero-signals intro-line intro-6">
              {heroSignals.map((item) => (
                <article className="signal-card" key={item.label}>
                  <p className="micro-label">{item.label}</p>
                  <p>{item.text}</p>
                </article>
              ))}
            </div>
          </div>

          <div className="hero-media">
            <section className="video-card" id="demo">
              <div className="video-card-header">
                <div>
                  <p className="eyebrow">Demo Video</p>
                  <p className="video-title">Temporary YouTube embed placeholder</p>
                </div>
                <span className="inline-note">Replace URL later</span>
              </div>

              <div className="video-frame">
                <iframe
                  src="https://www.youtube.com/embed/dQw4w9WgXcQ?rel=0"
                  title="Imprint demo placeholder video"
                  allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture; web-share"
                  referrerPolicy="strict-origin-when-cross-origin"
                  allowFullScreen
                />
              </div>

              <p className="video-note">
                Swap the iframe `src` in `frontend/src/App.tsx` when the real product demo is ready.
              </p>
            </section>

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

        <section className="product-grid" data-reveal>
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

        <section className="panel feature-panel" data-reveal>
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

        <section className="state-grid" id="statefulness" data-reveal>
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

        <section className="panel feature-panel" data-reveal>
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

        <section className="panel download-panel" id="download" data-reveal>
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
