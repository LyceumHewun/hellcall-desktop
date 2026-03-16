import { useState } from 'react';
import { KeySequence } from './KeySequence';
import { Trash2, GripVertical, Keyboard, ChevronDown, ChevronUp } from 'lucide-react';

interface MacroCardProps {
  id?: string;
  initialData?: {
    voiceTrigger: string;
    engineGrammar: string;
    responseAudio: string;
  };
  onDelete?: () => void;
}

export function MacroCard({ initialData, onDelete }: MacroCardProps) {
  const [formData, setFormData] = useState(
    initialData || {
      voiceTrigger: 'orbital strike',
      engineGrammar: '[orbital] [strike|bombardment]',
      responseAudio: 'confirm_orbital.wav',
    }
  );
  const [isExpanded, setIsExpanded] = useState(false);
  const [isRecording, setIsRecording] = useState(false);

  return (
    <div className="bg-[#1E2128] rounded border border-white/10 overflow-hidden">
      {/* Collapsed Row */}
      <div className="flex items-center gap-3 px-4 h-16 hover:bg-white/[0.02] transition-colors">
        {/* Drag Handle */}
        <button className="text-white/30 hover:text-white/60 transition-colors cursor-grab active:cursor-grabbing">
          <GripVertical className="w-4 h-4" />
        </button>

        {/* Voice Trigger Input */}
        <input
          type="text"
          value={formData.voiceTrigger}
          onChange={(e) => setFormData({ ...formData, voiceTrigger: e.target.value })}
          className="w-[20%] min-w-[140px] bg-transparent border-b border-white/10 px-2 py-1 text-white placeholder:text-white/30 focus:outline-none focus:border-[#FCE100]/60 transition-colors text-sm"
          placeholder="voice trigger"
        />

        {/* Macro Sequence */}
        <div className="flex-1 flex items-center">
          <KeySequence compact />
        </div>

        {/* Action Icons */}
        <div className="flex items-center gap-1">
          <button
            onClick={() => setIsRecording(!isRecording)}
            className={`p-2 rounded transition-colors ${
              isRecording
                ? 'bg-[#D93A3A]/20 text-[#D93A3A] border border-[#D93A3A]'
                : 'text-white/40 hover:text-[#FCE100] hover:bg-[#FCE100]/5'
            }`}
            title="Record Sequence"
          >
            <Keyboard className="w-4 h-4" />
          </button>

          <button
            onClick={() => setIsExpanded(!isExpanded)}
            className="p-2 text-white/40 hover:text-[#FCE100] hover:bg-[#FCE100]/5 rounded transition-colors"
            title="Advanced Settings"
          >
            {isExpanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
          </button>

          <button
            onClick={onDelete}
            className="p-2 text-white/40 hover:text-[#D93A3A] hover:bg-[#D93A3A]/5 rounded transition-colors"
            title="Delete Macro"
          >
            <Trash2 className="w-4 h-4" />
          </button>
        </div>
      </div>

      {/* Expanded Panel */}
      {isExpanded && (
        <div className="bg-[#15171C] border-t border-white/10 border-l-2 border-l-[#FCE100] px-4 py-4">
          <div className="grid grid-cols-2 gap-4">
            {/* Engine Grammar */}
            <div className="flex flex-col gap-1.5">
              <label className="text-white/50 text-xs uppercase tracking-wide">Engine Grammar</label>
              <input
                type="text"
                value={formData.engineGrammar}
                onChange={(e) => setFormData({ ...formData, engineGrammar: e.target.value })}
                className="bg-white/5 border border-white/10 rounded px-3 py-2 text-white text-sm placeholder:text-white/30 focus:outline-none focus:border-[#FCE100]/50 transition-colors font-mono"
                placeholder="[word] [option1|option2]"
              />
            </div>

            {/* Response Audio */}
            <div className="flex flex-col gap-1.5">
              <label className="text-white/50 text-xs uppercase tracking-wide">Response Audio</label>
              <input
                type="text"
                value={formData.responseAudio}
                onChange={(e) => setFormData({ ...formData, responseAudio: e.target.value })}
                className="bg-white/5 border border-white/10 rounded px-3 py-2 text-white text-sm placeholder:text-white/30 focus:outline-none focus:border-[#FCE100]/50 transition-colors"
                placeholder="audio_file.wav"
              />
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
