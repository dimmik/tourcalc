import React, { Component, Fragment } from "react";
import PropTypes from "prop-types";
import TextField from '@material-ui/core/TextField';

class Autocomplete extends Component {
    static propTypes = {
        suggestions: PropTypes.instanceOf(Array)
    };

    static defaultProps = {
        suggestions: []
    };

    constructor(props) {
        super(props);

        this.wrapperRef = React.createRef();
        
        //this.setWrapperRef = this.setWrapperRef.bind(this);
        this.handleClickOutside = this.handleClickOutside.bind(this);

        this.hint = props.hint;
        this.onFill = props.onFill;
        this.defaultValue = props.defaultValue;

        this.state = {
            // The active selection's index
            activeSuggestion: 0,
            // The suggestions that match the user's input
            filteredSuggestions: [],
            // Whether or not the suggestion list is shown
            showSuggestions: false,
            // What the user has entered
            userInput: this.defaultValue
        };
        //alert("wr: " + this.wrapperRef);
    }

    onChange = e => {
        const txt = e.currentTarget.value;
        this.filterSuggestions(txt);
    };
    filterSuggestions = (text) => {
        const { suggestions } = this.props;
        const userInput = text;
        // Filter our suggestions that don't contain the user's input
        const filteredSuggestions = suggestions.filter(
            suggestion =>
                suggestion.toLowerCase().indexOf(userInput.toLowerCase()) > -1
        );

        this.setState({
            activeSuggestion: 0,
            filteredSuggestions,
            showSuggestions: true,
            userInput: text
        });
    }

    onClickOnSuggestion = e => {
        //alert("s clicked: " + e.currentTarget.innerText);
        this.setState({
            activeSuggestion: 0,
            filteredSuggestions: [],
            showSuggestions: false,
            userInput: e.currentTarget.innerText
        });
    };

    onKeyDown = e => {
        const { activeSuggestion, filteredSuggestions } = this.state;

        // User pressed the enter key
        if (e.keyCode === 13) {
            this.setState({
                activeSuggestion: 0,
                showSuggestions: false,
                userInput: filteredSuggestions[activeSuggestion]
            });
        }
        
    };

    componentDidUpdate() {
        this.onFill(this.state.userInput);
    }

    componentDidMount() {
        document.addEventListener('mousedown', this.handleClickOutside);
    }

    componentWillUnmount() {
        document.removeEventListener('mousedown', this.handleClickOutside);
    }
    /**
     * Alert if clicked on outside of element
     */
    handleClickOutside(event) {
        var c = this.wrapperRef.current;
        if (this.wrapperRef && this.wrapperRef.current && !this.wrapperRef.current.contains(event.target)) {
            //alert('You clicked outside of me!');
            this.setState({ showSuggestions: false });
            this.wrapperRef.current = c;
        }
        //alert("wr: " + this.wrapperRef.current /*+ "outside: " + !this.wrapperRef.current.contains(event.target) */);
        //alert('You clicked! wr: ' + this.wrapperRef);
    }

    render() {
        const {
            onChange,
            onClickOnSuggestion,
            onKeyDown,
            state: {
                activeSuggestion,
                filteredSuggestions,
                showSuggestions,
                userInput
            }
        } = this;

        let suggestionsListComponent;

        if (showSuggestions) {
            if (filteredSuggestions.length > 0) {
                suggestionsListComponent = (
                    <ul className="suggestions">
                        {filteredSuggestions.map((suggestion, index) => {
                            let className;

                            // Flag the active suggestion with a class
                            if (index === activeSuggestion) {
                                className = "suggestion-active";
                            }

                            return (
                                <li className={className} key={suggestion} onClick={onClickOnSuggestion}>
                                    {suggestion}
                                </li>
                            );
                        })}
                    </ul>
                );
            } else {
                suggestionsListComponent = (
                    <div className="no-suggestions">
                        <em>No suggestions</em>
                    </div>
                );
            }
        }

        return (
            <Fragment>
                <div ref={this.wrapperRef}>
                    <TextField
                        id="type"
                        value={userInput}
                        onChange={onChange}
                        onKeyDown={onKeyDown}
                        onClick={() => this.filterSuggestions(this.state.userInput)}
                    
                        margin="normal"
                        autoComplete="off"
                        label={ this.hint }
                    />
                    {suggestionsListComponent}
                 </div>
            </Fragment>
        );
    }
}

export default Autocomplete;
