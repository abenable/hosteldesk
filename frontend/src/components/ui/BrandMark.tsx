import logo from '../../assets/logo.png';
import { cx } from './cx';

const SIZES = { sm: 'size-9', lg: 'mx-auto size-14' };

/** The HostelDesk logo (the app icon, in full colour). Decorative: the product or hostel name sits beside it. */
export function BrandMark({ size = 'sm' }: { size?: keyof typeof SIZES }) {
  return <img src={logo} alt="" className={cx('shrink-0 object-contain', SIZES[size])} />;
}
