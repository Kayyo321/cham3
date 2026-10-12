// Rebuild the owner-approved raster variants on macOS without redrawing the logo.
// Run from the repository root: swift assets/branding/refresh_artwork.swift
import Foundation
import CoreGraphics
import ImageIO
import UniformTypeIdentifiers

func load_image(_ path: String) -> CGImage {
    let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil)!
    return CGImageSourceCreateImageAtIndex(source, 0, nil)!
}

func image_bytes(_ image: CGImage) -> [UInt8] {
    var pixels = [UInt8](repeating: 0, count: image.width * image.height * 4)
    pixels.withUnsafeMutableBytes { buffer in
        let context = CGContext(data: buffer.baseAddress, width: image.width,
            height: image.height, bitsPerComponent: 8, bytesPerRow: image.width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
    }
    return pixels
}

func make_image(_ pixels: [UInt8], _ width: Int, _ height: Int, opaque: Bool = false) -> CGImage {
    let provider = CGDataProvider(data: Data(pixels) as CFData)!
    return CGImage(width: width, height: height, bitsPerComponent: 8,
        bitsPerPixel: 32, bytesPerRow: width * 4,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGBitmapInfo(rawValue: opaque ? CGImageAlphaInfo.noneSkipLast.rawValue : CGImageAlphaInfo.premultipliedLast.rawValue),
        provider: provider, decode: nil, shouldInterpolate: true, intent: .defaultIntent)!
}

func save_image(_ image: CGImage, _ path: String) {
    let destination = CGImageDestinationCreateWithURL(URL(fileURLWithPath: path) as CFURL,
        UTType.png.identifier as CFString, 1, nil)!
    CGImageDestinationAddImage(destination, image, nil)
    precondition(CGImageDestinationFinalize(destination))
}

func alpha_bounds(_ pixels: [UInt8], _ width: Int, _ height: Int) -> CGRect {
    var min_x = width
    var min_y = height
    var max_x = 0
    var max_y = 0
    for y in 0..<height {
        for x in 0..<width where pixels[(y * width + x) * 4 + 3] > 0 {
            min_x = min(min_x, x)
            min_y = min(min_y, y)
            max_x = max(max_x, x)
            max_y = max(max_y, y)
        }
    }
    return CGRect(x: min_x, y: min_y, width: max_x - min_x + 1, height: max_y - min_y + 1)
}

let source_image = load_image("assets/branding/cham3-logo.png")
let width = source_image.width
let height = source_image.height
precondition(width == 1254 && height == 1254, "Recipe is specific to the approved source.")
let source_pixels = image_bytes(source_image)
var logo_pixels = source_pixels
// Source coordinates: preserve the three dark eye rings as artwork.
let eye_circles: [(Double, Double, Double)] = [(749, 292, 32.5), (382.5, 534, 33), (887.5, 622, 32)]
var changed_interior = 0
for y in 0..<height {
    for x in 0..<width {
        let offset = (y * width + x) * 4
        let brightness = max(source_pixels[offset], source_pixels[offset + 1], source_pixels[offset + 2])
        var coverage = brightness >= 70 ? 1.0 : 0.0
        // Estimate edge coverage from the nearest opaque source pixel, then
        // remove the charcoal matte in premultiplied space. No new artwork.
        if brightness > 28 && brightness < 70 {
            var closest_distance = Int.max
            var foreground_peak = 0.0
            for dy in -2...2 {
                for dx in -2...2 {
                    let next_x = x + dx
                    let next_y = y + dy
                    if next_x < 0 || next_x >= width || next_y < 0 || next_y >= height { continue }
                    let next_offset = (next_y * width + next_x) * 4
                    let next_peak = max(source_pixels[next_offset], source_pixels[next_offset + 1], source_pixels[next_offset + 2])
                    let distance = dx * dx + dy * dy
                    if next_peak >= 70 && distance < closest_distance {
                        closest_distance = distance
                        foreground_peak = Double(next_peak)
                    }
                }
            }
            if foreground_peak > 0 {
                coverage = max(0, min(1, (Double(brightness) - 25) / (foreground_peak - 25)))
            }
        }
        for (eye_x, eye_y, radius) in eye_circles {
            let distance = hypot(Double(x) + 0.5 - eye_x, Double(y) + 0.5 - eye_y)
            coverage = max(coverage, max(0, min(1, radius + 0.5 - distance)))
        }
        logo_pixels[offset + 3] = UInt8((coverage * 255).rounded())
        for channel in 0..<3 {
            let matte = channel == 2 ? 25.0 : 22.0
            logo_pixels[offset + channel] = UInt8(max(0, min(coverage * 255,
                Double(source_pixels[offset + channel]) - (1 - coverage) * matte)).rounded())
            if brightness >= 70 && logo_pixels[offset + channel] != source_pixels[offset + channel] {
                changed_interior += 1
            }
        }
    }
}
precondition(changed_interior == 0, "Opaque source colors must be preserved.")
let logo_image = make_image(logo_pixels, width, height)
save_image(logo_image, "assets/branding/cham3-logo-transparent.png")
let logo_bounds = alpha_bounds(logo_pixels, width, height)

// The wordmark begins below row 870; the emblem ends at row 844.
var emblem_pixels = logo_pixels
for y in 860..<height {
    for x in 0..<width {
        let offset = (y * width + x) * 4
        for channel in 0..<4 { emblem_pixels[offset + channel] = 0 }
    }
}
let emblem_bounds = alpha_bounds(emblem_pixels, width, height)
let icon_side = Int(max(emblem_bounds.width, emblem_bounds.height)) + 48
var icon_pixels = [UInt8](repeating: 0, count: icon_side * icon_side * 4)
let icon_x = (icon_side - Int(emblem_bounds.width)) / 2
let icon_y = (icon_side - Int(emblem_bounds.height)) / 2
for y in 0..<Int(emblem_bounds.height) {
    for x in 0..<Int(emblem_bounds.width) {
        let from_offset = ((y + Int(emblem_bounds.minY)) * width + x + Int(emblem_bounds.minX)) * 4
        let to_offset = ((y + icon_y) * icon_side + x + icon_x) * 4
        for channel in 0..<4 { icon_pixels[to_offset + channel] = emblem_pixels[from_offset + channel] }
    }
}
save_image(make_image(icon_pixels, icon_side, icon_side), "assets/branding/cham3-logo-icon-transparent.png")

let wallpaper_width = 3840
let wallpaper_height = 2160
let cropped_logo = logo_image.cropping(to: logo_bounds)!
let logo_height = 680.0
let logo_width = logo_height * Double(logo_bounds.width) / Double(logo_bounds.height)
let logo_rect = CGRect(x: (Double(wallpaper_width) - logo_width) / 2,
    y: (Double(wallpaper_height) - logo_height) / 2, width: logo_width, height: logo_height)
var wallpaper_pixels = [UInt8](repeating: 0, count: wallpaper_width * wallpaper_height * 4)
wallpaper_pixels.withUnsafeMutableBytes { buffer in
    let context = CGContext(data: buffer.baseAddress, width: wallpaper_width,
        height: wallpaper_height, bitsPerComponent: 8, bytesPerRow: wallpaper_width * 4,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.setFillColor(CGColor(colorSpace: CGColorSpace(name: CGColorSpace.sRGB)!,
        components: [23.0 / 255, 23.0 / 255, 27.0 / 255, 1])!)
    context.fill(CGRect(x: 0, y: 0, width: wallpaper_width, height: wallpaper_height))
    context.interpolationQuality = .high
    context.draw(cropped_logo, in: logo_rect)
}
save_image(make_image(wallpaper_pixels, wallpaper_width, wallpaper_height, opaque: true),
    "assets/wallpapers/cham3_minimal_charcoal.png")
var background_pixels_checked = 0
for y in 0..<wallpaper_height {
    for x in 0..<wallpaper_width {
        let offset = (y * wallpaper_width + x) * 4
        precondition(wallpaper_pixels[offset + 3] == 255)
        if !logo_rect.insetBy(dx: -2, dy: -2).contains(CGPoint(x: x, y: y)) {
            precondition(Array(wallpaper_pixels[offset..<(offset + 3)]) == [23, 23, 27])
            background_pixels_checked += 1
        }
    }
}
print("PASS: source opaque artwork colors preserved; changed channels = \(changed_interior)")
print("PASS: full logo bounds = \(logo_bounds); emblem bounds = \(emblem_bounds); square icon = \(icon_side)")
print("PASS: opaque wallpaper 3840x2160; logo center (1920,1080), height 680; uniform background pixels = \(background_pixels_checked)")
