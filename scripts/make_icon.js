ObjC.import("AppKit");
ObjC.import("Foundation");

const size = 1024;
const image = $.NSImage.alloc.initWithSize($.NSMakeSize(size, size));
image.lockFocus;

function color(r, g, b, a) {
  return $.NSColor.colorWithCalibratedRedGreenBlueAlpha(r / 255, g / 255, b / 255, a);
}

color(255, 255, 255, 1).setFill;
$.NSBezierPath.bezierPathWithRoundedRectXRadiusYRadius($.NSMakeRect(0, 0, size, size), 210, 210).fill;

function makePath(points) {
  const p = $.NSBezierPath.bezierPath;
  p.moveToPoint($.NSMakePoint(points[0][0], size - points[0][1]));
  for (let i = 1; i < points.length; i++) {
    p.lineToPoint($.NSMakePoint(points[i][0], size - points[i][1]));
  }
  p.closePath;
  return p;
}

const outer = [
  [206, 694], [178, 649], [357, 263], [441, 263], [524, 441],
  [606, 263], [691, 263], [870, 649], [842, 694], [757, 694],
  [716, 667], [650, 519], [582, 666], [538, 694], [510, 694],
  [466, 666], [398, 519], [332, 667], [290, 694],
];

const shadow = $.NSShadow.alloc.init;
shadow.setShadowColor(color(39, 54, 77, 0.18));
shadow.setShadowOffset($.NSMakeSize(0, -26));
shadow.setShadowBlurRadius(32);
shadow.set;

const gradient = $.NSGradient.alloc.initWithColors($([
  color(117, 167, 255, 1),
  color(122, 92, 255, 1),
  color(69, 210, 213, 1),
]));
gradient.drawInBezierPathAngle(makePath(outer), -45);

const inner = [
  [433, 694], [312, 694], [438, 420], [490, 420], [524, 494],
  [558, 420], [610, 420], [736, 694], [616, 694], [524, 496],
];
const innerGradient = $.NSGradient.alloc.initWithColors($([
  color(131, 103, 255, 0.9),
  color(95, 184, 255, 0.9),
]));
innerGradient.drawInBezierPathAngle(makePath(inner), -80);

image.unlockFocus;

const tiff = image.TIFFRepresentation;
const rep = $.NSBitmapImageRep.alloc.initWithData(tiff);
const data = rep.representationUsingTypeProperties($.NSBitmapImageFileTypePNG, $({}));
data.writeToFileAtomically("src-tauri/icons/icon.png", true);
