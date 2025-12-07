#!/bin/bash
# Create a minimal 32x32 PNG icon
# This is a placeholder - replace with actual icon for production
python3 << 'PYTHON'
try:
    from PIL import Image
    img = Image.new('RGB', (32, 32), color='#4F46E5')
    img.save('icon.png')
    print("Created icon.png")
except ImportError:
    # Fallback: create a minimal valid PNG using ImageMagick or just create empty
    print("PIL not available")
PYTHON
