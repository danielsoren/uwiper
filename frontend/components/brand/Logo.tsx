// The UWiper mark: a W whose last stroke turns into a broom. The source is
// assets/logo.svg, which the app icons are generated from as well.
import logo from '../../../assets/logo.svg';

export function Logo({ size = 30 }: { size?: number }) {
  return <img src={logo} width={size} height={size} alt="" draggable={false} className="flex-none" />;
}
