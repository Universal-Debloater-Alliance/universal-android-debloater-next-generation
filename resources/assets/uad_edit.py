import json 


with open("uad_lists2.json", "r") as file:
    data = json.load(file)
    for app_id, app_data in data.items():

        # Removing some useless data
        if 'neededBy' in app_data:
            del app_data['neededBy']

        if 'labels' in app_data:
            del app_data['labels']

        if 'dependencies' in app_data: 
            del app_data['dependencies']

        # Renaming old data 
        removal = app_data.get('removal')
        if removal == "Recommended":
            app_data['removal'] = "Safe"
        
        elif removal == "Expert":
            app_data['removal'] = "Disruptive"


    with open("uad_listNew.json", "w") as new_file:
        json.dump(data, new_file, indent=2, ensure_ascii=False)

# print(data)
