export interface SectionHeaderProps {
  readonly eyebrow: string
  readonly title: string
}

export function SectionHeader({ eyebrow, title }: Readonly<SectionHeaderProps>) {
  return (
    <div className="section-header">
      <p className="eyebrow">{eyebrow}</p>
      <h2>{title}</h2>
    </div>
  )
}
