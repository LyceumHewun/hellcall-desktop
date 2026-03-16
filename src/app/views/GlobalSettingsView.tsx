import { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '../components/ui/card';
import { Slider } from '../components/ui/slider';
import { Label } from '../components/ui/label';
import { Input } from '../components/ui/input';

export function GlobalSettingsView() {
  const [settings, setSettings] = useState({
    chunk_time: 0.5,
    vad_silence_duration: 200,
    wait_open_time: 100,
    key_release_interval: 50,
    diff_key_interval: 20,
    hit_word: 'hello',
    hit_word_grammar: '[hello]',
  });

  return (
    <div className="flex-1 overflow-y-auto p-6 space-y-6">
      <div className="max-w-4xl mx-auto space-y-6 text-white">
        <Card className="bg-[#1E2128] border-white/10 text-white">
          <CardHeader>
            <CardTitle className="text-[#FCE100]">Recognizer</CardTitle>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="space-y-2">
              <Label>Chunk Time ({settings.chunk_time})</Label>
              <Slider
                value={[settings.chunk_time]}
                min={0.1}
                max={1.0}
                step={0.1}
                onValueChange={([val]) => setSettings(s => ({...s, chunk_time: val}))}
              />
            </div>
            <div className="space-y-2">
              <Label>VAD Silence Duration ({settings.vad_silence_duration})</Label>
              <Slider
                value={[settings.vad_silence_duration]}
                min={50}
                max={500}
                step={10}
                onValueChange={([val]) => setSettings(s => ({...s, vad_silence_duration: val}))}
              />
            </div>
          </CardContent>
        </Card>

        <Card className="bg-[#1E2128] border-white/10 text-white">
          <CardHeader>
            <CardTitle className="text-[#FCE100]">Key Presser</CardTitle>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="space-y-2">
              <Label>Wait Open Time</Label>
              <Input
                type="number"
                className="bg-black/30 border-white/10"
                value={settings.wait_open_time}
                onChange={(e) => setSettings(s => ({...s, wait_open_time: Number(e.target.value)}))}
              />
            </div>
            <div className="space-y-2">
              <Label>Key Release Interval</Label>
              <Input
                type="number"
                className="bg-black/30 border-white/10"
                value={settings.key_release_interval}
                onChange={(e) => setSettings(s => ({...s, key_release_interval: Number(e.target.value)}))}
              />
            </div>
            <div className="space-y-2">
              <Label>Diff Key Interval</Label>
              <Input
                type="number"
                className="bg-black/30 border-white/10"
                value={settings.diff_key_interval}
                onChange={(e) => setSettings(s => ({...s, diff_key_interval: Number(e.target.value)}))}
              />
            </div>
          </CardContent>
        </Card>

        <Card className="bg-[#1E2128] border-white/10 text-white">
          <CardHeader>
            <CardTitle className="text-[#FCE100]">Trigger</CardTitle>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="space-y-2">
              <Label>Hit Word</Label>
              <Input
                className="bg-black/30 border-white/10"
                value={settings.hit_word}
                onChange={(e) => setSettings(s => ({...s, hit_word: e.target.value}))}
              />
            </div>
            <div className="space-y-2">
              <Label>Hit Word Grammar</Label>
              <Input
                className="bg-black/30 border-white/10"
                value={settings.hit_word_grammar}
                onChange={(e) => setSettings(s => ({...s, hit_word_grammar: e.target.value}))}
              />
            </div>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
