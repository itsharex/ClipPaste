import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { ClipCard } from '@/components/ClipCard';
import type { ClipboardItem } from '@/types';

const makeClip = (overrides: Partial<ClipboardItem> = {}): ClipboardItem => ({
  id: '1',
  clip_type: 'text',
  content: 'Hello World',
  preview: 'Hello World',
  folder_id: null,
  created_at: '2024-01-01T00:00:00Z',
  source_app: 'VS Code',
  source_icon: null,
  metadata: null,
  is_pinned: false,
  subtype: null,
  note: null,
  paste_count: 0,
  ...overrides,
});

describe('ClipCard', () => {
  const defaultProps = {
    clip: makeClip(),
    isSelected: false,
    onSelect: vi.fn(),
    onPaste: vi.fn(),
    onCopy: vi.fn(),
    onPin: vi.fn(),
  };

  it('renders text content', () => {
    render(<ClipCard {...defaultProps} />);
    expect(screen.getByText('Hello World')).toBeInTheDocument();
  });

  it('shows source app name in header', () => {
    render(<ClipCard {...defaultProps} />);
    expect(screen.getByText('VS Code')).toBeInTheDocument();
  });

  it('shows clip_type when no source_app', () => {
    const clip = makeClip({ source_app: null });
    render(<ClipCard {...defaultProps} clip={clip} />);
    expect(screen.getByText('TEXT')).toBeInTheDocument();
  });

  it('calls onSelect on click', () => {
    const onSelect = vi.fn();
    render(<ClipCard {...defaultProps} onSelect={onSelect} />);
    fireEvent.click(screen.getByText('Hello World').closest('[data-clip-id]')!.querySelector('[draggable]')!);
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it('calls onPaste on double click', () => {
    const onPaste = vi.fn();
    render(<ClipCard {...defaultProps} onPaste={onPaste} />);
    const card = screen.getByText('Hello World').closest('[data-clip-id]')!.querySelector('[draggable]')!;
    fireEvent.doubleClick(card);
    expect(onPaste).toHaveBeenCalledTimes(1);
  });

  it('shows character count for text clips', () => {
    render(<ClipCard {...defaultProps} />);
    expect(screen.getByText('11 chars')).toBeInTheDocument();
  });

  it('shows image size for image clips', () => {
    // base64 for a tiny 1x1 PNG (approx)
    const clip = makeClip({
      clip_type: 'image',
      content: 'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==',
    });
    render(<ClipCard {...defaultProps} clip={clip} />);
    // Should show "XKB" format
    const sizeText = screen.getByText(/^\d+KB$/);
    expect(sizeText).toBeInTheDocument();
  });

  it('renders image element for image clips', () => {
    const clip = makeClip({
      clip_type: 'image',
      content: 'dGVzdA==',
    });
    render(<ClipCard {...defaultProps} clip={clip} />);
    const img = screen.getByRole('img', { name: 'Clipboard Image' });
    expect(img).toBeInTheDocument();
    expect(img.getAttribute('src')).toContain('data:image/png;base64,');
  });

  it('truncates long text content to PREVIEW_CHAR_LIMIT', () => {
    const longContent = 'A'.repeat(500);
    const clip = makeClip({ content: longContent });
    const { container } = render(<ClipCard {...defaultProps} clip={clip} />);
    // ClipCard uses substring(0, PREVIEW_CHAR_LIMIT=300)
    const span = container.querySelector('pre span');
    expect(span?.textContent?.length).toBe(300);
  });

  it('has data-clip-id attribute', () => {
    const { container } = render(<ClipCard {...defaultProps} />);
    const el = container.querySelector('[data-clip-id="1"]');
    expect(el).toBeInTheDocument();
  });

  it('applies selected styling', () => {
    const { container } = render(<ClipCard {...defaultProps} isSelected={true} />);
    const card = container.querySelector('[draggable]');
    expect(card?.className).toContain('ring-blue-500');
  });

  it('shows pin button when showPin is true', () => {
    render(<ClipCard {...defaultProps} showPin={true} />);
    const pinButton = screen.getByTitle('Pin');
    expect(pinButton).toBeInTheDocument();
  });

  it('shows Unpin title when clip is pinned', () => {
    const clip = makeClip({ is_pinned: true });
    render(<ClipCard {...defaultProps} clip={clip} showPin={true} />);
    const pinButton = screen.getByTitle('Unpin');
    expect(pinButton).toBeInTheDocument();
  });

  it('calls onPin when pin button is clicked', () => {
    const onPin = vi.fn();
    render(<ClipCard {...defaultProps} onPin={onPin} showPin={true} />);
    fireEvent.click(screen.getByTitle('Pin'));
    expect(onPin).toHaveBeenCalledTimes(1);
  });

  it('calls onContextMenu on right-click', () => {
    const onContextMenu = vi.fn();
    render(<ClipCard {...defaultProps} onContextMenu={onContextMenu} />);
    const card = screen.getByText('Hello World').closest('[data-clip-id]')!.querySelector('[draggable]')!;
    fireEvent.contextMenu(card);
    expect(onContextMenu).toHaveBeenCalledTimes(1);
  });

  it('sets draggable attribute', () => {
    const { container } = render(<ClipCard {...defaultProps} />);
    const card = container.querySelector('[draggable="true"]');
    expect(card).toBeInTheDocument();
  });

  it('shows source icon when available', () => {
    const clip = makeClip({ source_icon: 'dGVzdA==' });
    render(<ClipCard {...defaultProps} clip={clip} />);
    const icons = document.querySelectorAll('img');
    const sourceIcon = Array.from(icons).find(img => img.src.includes('data:image/png;base64,dGVzdA=='));
    expect(sourceIcon).toBeDefined();
  });
});
