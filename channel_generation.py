# -*- coding: utf-8 -*-

from constants import AUTO_GENERATABLE_CHANNELS

def generate_missing_channels(hrir, auto_generate_config):
    """자동으로 누락된 채널들을 생성합니다.
    
    Args:
        hrir: HRIR 객체
        auto_generate_config: 자동 생성할 채널들의 설정 딕셔너리
        
    Returns:
        생성된 채널들의 리스트
    """
    generated_channels = []
    
    for channel_name, should_generate in auto_generate_config.items():
        # FC, TSL, TSR 채널은 강제 생성하지 않도록 조건 추가
        if should_generate and channel_name in AUTO_GENERATABLE_CHANNELS and channel_name not in ['FC', 'TSL', 'TSR']:
            config = AUTO_GENERATABLE_CHANNELS[channel_name]
            sources = config['sources']
            weights = config['weights']
            
            # 소스 채널들이 모두 존재하는지 확인
            if all(src in hrir.irs for src in sources):
                print(f'Generating {channel_name} from {sources} with weights {weights}')
                
                # 새 채널 생성
                hrir.irs[channel_name] = {}
                for side in ['left', 'right']:
                    # 가중 평균으로 새 채널 생성
                    mixed_data = None
                    for i, src in enumerate(sources):
                        src_data = hrir.irs[src][side].data * weights[i]
                        if mixed_data is None:
                            mixed_data = src_data
                        else:
                            mixed_data += src_data
                    
                    # 새 IR 객체 생성
                    from hrir import ImpulseResponse
                    new_ir = ImpulseResponse(mixed_data, hrir.fs)
                    hrir.irs[channel_name][side] = new_ir
                
                generated_channels.append(channel_name)
            else:
                missing_sources = [src for src in sources if src not in hrir.irs]
                print(f'Cannot generate {channel_name}: missing source channels {missing_sources}')
    
    return generated_channels

def get_available_channels_for_layout(hrir, layout_channels):
    """특정 레이아웃에 사용 가능한 채널들을 반환합니다.
    
    Args:
        hrir: HRIR 객체
        layout_channels: 레이아웃에 필요한 채널들의 리스트
        
    Returns:
        사용 가능한 채널들의 리스트
    """
    return [ch for ch in layout_channels if ch in hrir.irs]

def create_truehd_layout_track_order(available_channels):
    """TrueHD 레이아웃용 트랙 순서를 생성합니다.
    
    Args:
        available_channels: 사용 가능한 채널들의 리스트
        
    Returns:
        트랙 순서 리스트 (예: ['FL-left', 'FL-right', ...])
    """
    track_order = []
    for ch in available_channels:
        track_order.append(f'{ch}-left')
        track_order.append(f'{ch}-right')
    return track_order

def validate_channel_requirements(hrir, required_channels, min_channels=8):
    """채널 요구사항을 검증합니다.
    
    Args:
        hrir: HRIR 객체
        required_channels: 필요한 채널들의 리스트
        min_channels: 최소 필요한 채널 수
        
    Returns:
        (유효성, 사용 가능한 채널 수, 메시지)
    """
    available = [ch for ch in required_channels if ch in hrir.irs]
    available_count = len(available)
    
    if available_count >= min_channels:
        return True, available_count, f"Found {available_count} channels for layout"
    else:
        missing = [ch for ch in required_channels if ch not in hrir.irs]
        return False, available_count, f"Insufficient channels: need {min_channels}, have {available_count}. Missing: {missing}"

def print_channel_mapping_info(channel_info):
    """채널 매핑 정보를 출력합니다.
    
    Args:
        channel_info: 채널 정보 리스트
    """
    if channel_info:
        print("Channel mapping information:")
        for i, ch in enumerate(channel_info):
            print(f"  Channel {i+1}: {ch}")
    else:
        print("No channel mapping information available.") 