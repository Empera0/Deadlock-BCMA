import { Hero } from "../api";

interface Props {
  heroes: Hero[];
  selected: string;
  onPick: (className: string) => void;
  onClose: () => void;
}

export default function HeroPicker({ heroes, selected, onPick, onClose }: Props) {
  return (
    <div className="overlay" onClick={onClose}>
      <div className="hero-grid" onClick={(e) => e.stopPropagation()}>
        {heroes.map((h) => (
          <button
            key={h.class_name}
            className={"hero-cell" + (h.class_name === selected ? " selected" : "")}
            onClick={() => onPick(h.class_name)}
          >
            <img src={h.icon} alt="" loading="lazy" />
            <span>{h.name}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
